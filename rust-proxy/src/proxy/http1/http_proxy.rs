use crate::constants::common_constants::DEFAULT_HTTP_TIMEOUT;
use crate::monitor::prometheus_exporter::metrics;
use crate::proxy::http1::app_clients::AppClients;
use crate::proxy::proxy_trait::DestinationResult;
use crate::vojo::app_error::AppError;
use crate::vojo::cli::SharedConfig;
use bytes::Bytes;
use http::{HeaderValue, Uri};
use hyper::body::Incoming;
use hyper::header;
use hyper::header::{CONNECTION, SEC_WEBSOCKET_KEY};
use hyper::Method;
use hyper::StatusCode;

use crate::proxy::http1::websocket_proxy::server_upgrade;
use crate::proxy::proxy_trait::{ChainTrait, SpireContext};
use crate::proxy::proxy_trait::{CommonCheckRequest, RouterDestination};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_staticfile::Static;
use hyper_util::rt::TokioIo;
use rustls_pki_types::CertificateDer;
use serde_json::json;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
pub struct HttpProxy {
    pub port: i32,
    pub channel: mpsc::Receiver<()>,
    pub mapping_key: String,
    pub shared_config: SharedConfig,
}

impl HttpProxy {
    pub async fn start_http_server(&mut self) -> Result<(), AppError> {
        let port_clone = self.port;
        let addr = SocketAddr::from(([0, 0, 0, 0], port_clone as u16));
        let client = AppClients::new(self.shared_config.clone(), self.port).await?;
        let mapping_key_clone1 = self.mapping_key.clone();
        let reveiver = &mut self.channel;

        let listener = TcpListener::bind(addr).await?;
        info!("Listening on http://{addr}");
        loop {
            tokio::select! {
               Ok((stream,addr))= listener.accept()=>{
                let client_cloned = client.clone();
                let cloned_shared_config=self.shared_config.clone();
                let cloned_port=self.port;
                let mapping_key2 = mapping_key_clone1.clone();
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);

                    if let Err(err) = http1::Builder::new()
                    .preserve_header_case(true)
                    .title_case_headers(true)
                        .serve_connection(
                            io,
                            service_fn(move |req: Request<Incoming>| {
                                let req = req.map(|item| {
                                    item.map_err(AppError::from).boxed()
                                });
                                proxy_adapter(cloned_port,cloned_shared_config.clone(),client_cloned.clone(), req, mapping_key2.clone(), addr)
                            }),
                        )
                        .await
                    {
                        error!("Error serving connection: {err:?}");
                    }
                });
                },
                _ = reveiver.recv() => {
                    info!("http server stoped");
                    break;
                }
            }
        }

        Ok(())
    }
    pub async fn start_https_server(
        &mut self,
        pem_str: String,
        key_str: String,
    ) -> Result<(), AppError> {
        let port_clone = self.port;
        let addr = SocketAddr::from(([0, 0, 0, 0], port_clone as u16));
        let client = AppClients::new(self.shared_config.clone(), self.port).await?;
        let mapping_key_clone1 = self.mapping_key.clone();

        let mut cer_reader = BufReader::new(pem_str.as_bytes());
        let certs: Vec<CertificateDer<'_>> =
            rustls_pemfile::certs(&mut cer_reader).collect::<Result<Vec<_>, _>>()?;

        let mut key_reader = BufReader::new(key_str.as_bytes());
        let key_der = rustls_pemfile::private_key(&mut key_reader)?
            .ok_or_else(|| AppError("Key not found in PEM file".to_string()))?;

        let tls_cfg = {
            let cfg = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key_der)?;
            Arc::new(cfg)
        };
        let tls_acceptor = TlsAcceptor::from(tls_cfg);
        let reveiver = &mut self.channel;

        let listener = TcpListener::bind(addr).await?;
        info!("Listening on http://{addr}");
        loop {
            tokio::select! {
                    Ok((tcp_stream,addr))= listener.accept()=>{
                let tls_acceptor = tls_acceptor.clone();
                let cloned_shared_config=self.shared_config.clone();
                let cloned_port=self.port;
                let client = client.clone();
                let mapping_key2 = mapping_key_clone1.clone();
                tokio::spawn(async move {
                    let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                        Ok(tls_stream) => tls_stream,
                        Err(err) => {
                            error!("failed to perform tls handshake: {err:#}");
                            return;
                        }
                    };
                    let io = TokioIo::new(tls_stream);
                    let service = service_fn(move |req: Request<Incoming>| {
                        let req = req
                            .map(|item| item.map_err(AppError::from).boxed());

                        proxy_adapter(cloned_port,cloned_shared_config.clone(),client.clone(), req, mapping_key2.clone(), addr)
                    });
                    if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                        error!("Error serving connection: {err:?}");
                    }
                });
            },
                    _ = reveiver.recv() => {
                        info!("https server stoped");
                        break;
                    }
                }
        }

        Ok(())
    }
}
async fn proxy_adapter(
    port: i32,
    shared_config: SharedConfig,
    client: AppClients,
    req: Request<BoxBody<Bytes, AppError>>,
    mapping_key: String,
    remote_addr: SocketAddr,
) -> Result<Response<BoxBody<Bytes, AppError>>, AppError> {
    let result =
        proxy_adapter_with_error(port, shared_config, client, req, mapping_key, remote_addr).await;
    match result {
        Ok(res) => Ok(res),
        Err(err) => {
            error!("The error is {err}.");
            let json_value = json!({
                "error": err.to_string(),
            });
            Ok(Response::builder().status(StatusCode::NOT_FOUND).body(
                Full::new(Bytes::copy_from_slice(json_value.to_string().as_bytes()))
                    .map_err(AppError::from)
                    .boxed(),
            )?)
        }
    }
}
async fn proxy_adapter_with_error(
    port: i32,
    shared_config: SharedConfig,
    client: AppClients,
    req: Request<BoxBody<Bytes, AppError>>,
    mapping_key: String,
    remote_addr: SocketAddr,
) -> Result<Response<BoxBody<Bytes, AppError>>, AppError> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/")
        .to_string();

    let current_time = SystemTime::now();

    let timer = metrics::HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[mapping_key.as_str(), path.as_str(), method.as_str()])
        .start_timer();

    let res = proxy(
        port,
        shared_config,
        client,
        req,
        mapping_key.clone(),
        remote_addr,
        CommonCheckRequest {},
    )
    .await
    .unwrap_or_else(|err| {
        error!("The error is {err}.");
        let json_value = json!({
            "response_code": -1,
            "response_object": format!("{}", err)
        });
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(
                Full::new(Bytes::copy_from_slice(json_value.to_string().as_bytes()))
                    .map_err(AppError::from)
                    .boxed(),
            )
            .unwrap()
    });
    timer.observe_duration();
    let status = res.status();
    metrics::HTTP_REQUESTS_TOTAL
        .with_label_values(&[
            mapping_key.as_str(),
            &path,
            method.as_str(),
            status.as_str(),
        ])
        .inc();

    let elapsed_time_res = current_time.elapsed()?;
    info!(
        "{} - -  \"{} {} HTTP/1.1\" {}  \"-\" \"-\"  {:?}",
        remote_addr,
        method,
        path,
        status.as_u16(),
        elapsed_time_res
    );
    Ok(res)
}

async fn proxy(
    port: i32,
    shared_config: SharedConfig,
    client: AppClients,
    mut req: Request<BoxBody<Bytes, AppError>>,
    mapping_key: String,
    remote_addr: SocketAddr,
    chain_trait: impl ChainTrait,
) -> Result<Response<BoxBody<Bytes, AppError>>, AppError> {
    debug!("req: {req:?}");

    let inbound_headers = req.headers();
    let uri = req.uri().clone();
    let mut spire_context = SpireContext::new(port, None);
    let handling_result = chain_trait
        .get_destination(
            shared_config.clone(),
            port,
            mapping_key.clone(),
            inbound_headers,
            uri,
            remote_addr,
            &mut spire_context,
        )
        .await?;
    debug!("The get_destination is {handling_result:?}");
    let handling_result = match handling_result {
        DestinationResult::Matched(hr) => hr,
        DestinationResult::NotAllowed(denial) => {
            debug!("Request denied: {denial:?}");
            let mut response = Response::builder().status(denial.status).body(
                Full::new(Bytes::from(denial.body))
                    .map_err(AppError::from)
                    .boxed(),
            )?;
            response.headers_mut().extend(denial.headers);
            return Ok(response);
        }
        DestinationResult::NoMatchFound => {
            debug!("No match found for the request.");
            let response = Response::builder().status(StatusCode::NOT_FOUND).body(
                Full::new(Bytes::from("Not Found"))
                    .map_err(AppError::from)
                    .boxed(),
            )?;
            return Ok(response);
        }
    };

    if req.method() == Method::OPTIONS
        && req.headers().contains_key(header::ORIGIN)
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
    {
        if let Some(cors_config) = spire_context.cors_configed()? {
            return chain_trait.handle_preflight(cors_config, "");
        }
    }
    if inbound_headers.clone().contains_key(CONNECTION)
        && inbound_headers.contains_key(SEC_WEBSOCKET_KEY)
    {
        debug!(
            "The request has been updated to websocket,the req is {req:?}!"
        );
        return server_upgrade(req, handling_result, client.http).await;
    }

    let check_request = handling_result;
    let request_path = check_request.request_path.as_str();
    let router_destination = check_request.router_destination;
    let mut res = match router_destination {
        RouterDestination::File(ref _s) => {
            let mut parts = req.uri().clone().into_parts();
            parts.path_and_query = Some(request_path.try_into()?);
            *req.uri_mut() = Uri::from_parts(parts)?;
            route_file(router_destination, req).await
        }
        RouterDestination::Http(_s) => {
            *req.uri_mut() = request_path.parse()?;
            let host = req
                .uri()
                .host()
                .ok_or("Uri to host cause error")?
                .to_string();
            req.headers_mut()
                .insert(http::header::HOST, HeaderValue::from_str(&host)?);
            if let Some(mut middlewares) = spire_context.middlewares.clone() {
                if !middlewares.is_empty() {
                    chain_trait
                        .handle_before_request(&mut middlewares, remote_addr, &mut req)
                        .await?;
                }
            }
            let request_future = if request_path.contains("https") {
                client.http.request_https(req, DEFAULT_HTTP_TIMEOUT)
            } else {
                client.http.request_http(req, DEFAULT_HTTP_TIMEOUT)
            };
            let response_result = match request_future.await {
                Ok(response) => response.map_err(AppError::from),
                _ => {
                    return Err(AppError(format!(
                        "Request time out,the uri is {request_path}"
                    )))
                }
            };
            response_result.map(|item| {
                item.map(|s| s.boxed())
                    .map(|item: BoxBody<Bytes, hyper::Error>| item.map_err(AppError::from).boxed())
            })
        }
        RouterDestination::Grpc(s) => {
            info!("The request is grpc!,{request_path}");
            let grpc_client = client
                .grpc
                .ok_or(AppError::from(""))?
                .get_client(&s.endpoint)
                .await?;

            let body_bytes = req.collect().await?.to_bytes();
            let parts: Vec<&str> = request_path.split('/').filter(|s| !s.is_empty()).collect();
            if parts.len() < 2 {
                return Err(AppError(request_path.to_string()));
            }
            let service_name = parts[0].to_string();
            let method_name = parts[1].to_string();
            let grpc_response = grpc_client
                .do_request(service_name, method_name, body_bytes)
                .await?;
            let dynamic_message = grpc_response.into_inner();
            let response_json_string = serde_json::to_string(&dynamic_message)?;
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(
                    Full::new(Bytes::from(response_json_string))
                        .map_err(|e| AppError(format!("Failed to create response body: {e}"))) // map_err 的类型是 Infallible，但为保持一致性仍可转换
                        .boxed(),
                )?;

            Ok(response)
        }
    };
    if let Some(mut middlewares) = spire_context.middlewares {
        if !middlewares.is_empty() {
            chain_trait
                .handle_before_response(&mut middlewares, request_path, &mut res)
                .await?;
        }
    }
    res
}

async fn route_file(
    router_destination: RouterDestination,
    req: Request<BoxBody<Bytes, AppError>>,
) -> Result<Response<BoxBody<Bytes, AppError>>, AppError> {
    let static_ = Static::new(Path::new(router_destination.get_endpoint().as_str()));
    static_
        .clone()
        .serve(req)
        .await
        .map(|item| {
            item.map(|body| {
                body.boxed()
                    .map_err(|_| -> AppError { unreachable!() })
                    .boxed()
            })
        })
        .map_err(AppError::from)
}
#[cfg(test)]
mod tests {
    use super::*;

    use crate::middleware::authentication::BasicAuth;
    use crate::middleware::middlewares::MiddleWares;
    use crate::proxy::proxy_trait::{HandlingResult, MockChainTrait};
    use crate::vojo::app_config::Matcher;
    use crate::vojo::app_config::{ApiService, RouteConfig};
    use crate::vojo::router::{BaseRoute, RandomRoute, Router};
    use crate::{vojo::router::StaticFileRoute, AppConfig};
    use http::HeaderMap;
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::net::Ipv4Addr;
    use std::sync::Mutex;

    #[test]
    fn test_http_proxy_creation() {
        let (_, rx) = mpsc::channel(1);
        let shared_config = SharedConfig {
            shared_data: Arc::new(Mutex::new(AppConfig::default())),
        };

        let proxy = HttpProxy {
            port: 8080,
            channel: rx,
            mapping_key: "test".to_string(),
            shared_config,
        };

        assert_eq!(proxy.port, 8080);
        assert_eq!(proxy.mapping_key, "test");
    }

    #[tokio::test]
    async fn test_proxy_adapter_error_handling() {
        let shared_config = SharedConfig {
            shared_data: Arc::new(Mutex::new(AppConfig::default())),
        };
        let client = AppClients::new(shared_config.clone(), 3302).await.unwrap();

        let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let req = Request::builder()
            .uri("invalid://uri")
            .body(
                Full::new(Bytes::from("test"))
                    .map_err(AppError::from)
                    .boxed(),
            )
            .unwrap();

        let result = proxy_adapter(
            8080,
            shared_config,
            client,
            req,
            "test".to_string(),
            remote_addr,
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    #[tokio::test]
    async fn test_options_preflight_request() {
        // let _ = setup_logger_for_test();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("POST"),
        );

        let shared_config = SharedConfig {
            shared_data: Arc::new(Mutex::new(AppConfig {
                api_service_config: HashMap::from([(
                    8080,
                    ApiService {
                        listen_port: 8080,
                        route_configs: vec![RouteConfig {
                            router: Router::Random(RandomRoute {
                                routes: vec![BaseRoute {
                                    endpoint: "http://127.0.0.1:9394".to_string(),
                                    ..Default::default()
                                }],
                            }),
                            matcher: Some(Matcher {
                                prefix: "/".to_string(),
                                prefix_rewrite: "/".to_string(),
                            }),

                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            })),
        };
        let client = AppClients::new(shared_config.clone(), 8080).await.unwrap();

        let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut req = Request::builder()
            .method(Method::OPTIONS)
            .uri("http://127.0.0.1:8080/test")
            .body(Full::new(Bytes::from("")).map_err(AppError::from).boxed())
            .unwrap();
        req.headers_mut().extend(headers);

        let result = proxy(
            8080,
            shared_config,
            client,
            req,
            "test".to_string(),
            remote_addr,
            CommonCheckRequest {},
        )
        .await;
        println!("result is {result:?}");
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn test_proxy_handling_result_none() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("POST"),
        );

        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let client = AppClients::new(shared_config.clone(), 8080).await.unwrap();

        let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut req = Request::builder()
            .method(Method::OPTIONS)
            .uri("http://127.0.0.1:8080/test")
            .body(Full::new(Bytes::from("")).map_err(AppError::from).boxed())
            .unwrap();
        req.headers_mut().extend(headers);

        let mut mock_chain_trait = MockChainTrait::new();
        mock_chain_trait
            .expect_get_destination()
            .returning(|_, _, _, _, _, _, _| {
                Ok(crate::proxy::proxy_trait::DestinationResult::NoMatchFound)
            });
        let result = proxy(
            8080,
            shared_config,
            client,
            req,
            "test".to_string(),
            remote_addr,
            mock_chain_trait,
        )
        .await;
        println!("result is {result:?}");
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn test_proxy_middle() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("POST"),
        );

        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let client = AppClients::new(shared_config.clone(), 8080).await.unwrap();

        let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut req = Request::builder()
            .method(Method::OPTIONS)
            .uri("http://127.0.0.1:8080/test")
            .body(Full::new(Bytes::from("")).map_err(AppError::from).boxed())
            .unwrap();
        req.headers_mut().extend(headers);

        let mut mock_chain_trait = MockChainTrait::new();
        mock_chain_trait
            .expect_get_destination()
            .returning(|_, _, _, _, _, _, spire_context| {
                spire_context.middlewares = Some(vec![MiddleWares::Authentication(
                    crate::middleware::authentication::Authentication::Basic(BasicAuth {
                        credentials: "user:pass".to_string(),
                    }),
                )]);

                Ok(crate::proxy::proxy_trait::DestinationResult::Matched(
                    HandlingResult {
                        request_path: "/test".to_string(),
                        router_destination: RouterDestination::File(StaticFileRoute {
                            doc_root: "./test".to_string(),
                        }),
                    },
                ))
            });
        mock_chain_trait
            .expect_handle_before_request()
            .returning(|_, _, _| Err(AppError("test".to_string())));
        let result = proxy(
            8080,
            shared_config,
            client,
            req,
            "test".to_string(),
            remote_addr,
            mock_chain_trait,
        )
        .await;
        println!("result is {result:?}");
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn test_proxy_route_file() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("POST"),
        );

        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let client = AppClients::new(shared_config.clone(), 8080).await.unwrap();

        let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut req = Request::builder()
            .method(Method::OPTIONS)
            .uri("http://127.0.0.1:8080/test")
            .body(Full::new(Bytes::from("")).map_err(AppError::from).boxed())
            .unwrap();
        req.headers_mut().extend(headers);

        let mut mock_chain_trait = MockChainTrait::new();
        mock_chain_trait
            .expect_get_destination()
            .returning(|_, _, _, _, _, _, _| {
                Ok(crate::proxy::proxy_trait::DestinationResult::Matched(
                    HandlingResult {
                        request_path: "/test".to_string(),
                        router_destination: RouterDestination::File(StaticFileRoute {
                            doc_root: "./test".to_string(),
                        }),
                    },
                ))
            });
        mock_chain_trait
            .expect_handle_before_request()
            .returning(|_, _, _| Err(AppError("test".to_string())));
        let result = proxy(
            8080,
            shared_config,
            client,
            req,
            "test".to_string(),
            remote_addr,
            mock_chain_trait,
        )
        .await;
        println!("result is {result:?}");
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn test_route_file() {
        let router_destination = RouterDestination::File(StaticFileRoute {
            doc_root: "./test".to_string(),
        });

        let req = Request::builder()
            .uri("http://localhost/test.txt")
            .body(Full::new(Bytes::from("")).map_err(AppError::from).boxed())
            .unwrap();

        let result = route_file(router_destination, req).await;
        assert!(result.is_ok());
    }
}
