use crate::proxy::http1::grpc_client::GrpcClients;
use crate::proxy::http1::http_client::HttpClients;
use crate::vojo::cli::SharedConfig;
use crate::vojo::router::BaseRoute;
use crate::AppError;
#[derive(Clone)]
pub struct AppClients {
    pub http: HttpClients,
    pub grpc: Option<GrpcClients>,
}
impl AppClients {
    pub async fn new(shared_config: SharedConfig, port: i32) -> Result<AppClients, AppError> {
        let grpc_endpoints = {
            let lock = shared_config.shared_data.lock()?;
            let api_service = lock
                .api_service_config
                .get(&port)
                .ok_or_else(|| AppError::from("Can not find api service."))?;

            let mut grpc_list = vec![];
            for item in api_service.route_configs.iter() {
                let all_route = item.router.get_all_route()?.into_iter();
                let grpc_endpoints = all_route
                    .map(|x| x.endpoint)
                    .filter(|x| x.to_lowercase().starts_with("grpc://"))
                    .collect::<Vec<String>>();
                if grpc_endpoints.is_empty() {
                    continue;
                }
                for endpoint in grpc_endpoints {
                    grpc_list.push((
                        endpoint,
                        item.transcode
                            .clone()
                            .ok_or(AppError::from("Can not find transcode."))?
                            .proto_descriptor_set,
                    ));
                }
            }
            grpc_list
        };
        if grpc_endpoints.is_empty() {
            Ok(AppClients {
                http: HttpClients::new(),
                grpc: None,
            })
        } else {
            let grpc_clients = GrpcClients::new(grpc_endpoints).await?;
            Ok(AppClients {
                http: HttpClients::new(),
                grpc: Some(grpc_clients),
            })
        }
    }
}
