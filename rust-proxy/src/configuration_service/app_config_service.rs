use crate::app_error;
use crate::control_plane::certificate_manager::CertificateManager;
use crate::health_check::health_check_task::HealthCheck;
use crate::proxy::http1::http_proxy::HttpProxy;
use crate::proxy::http2::grpc_proxy::GrpcProxy;
use crate::proxy::tcp::tcp_proxy::TcpProxy;
use crate::vojo::app_config::ServiceType;
use crate::vojo::app_error::AppError;
use crate::vojo::cli::SharedConfig;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn init(shared_config: SharedConfig) -> Result<(), AppError> {
    let cloned_config = shared_config.clone();
    tokio::task::spawn(async {
        let mut health_check = HealthCheck::from_shared_config(cloned_config);
        health_check.start_health_check_loop().await;
    });
    let mut app_config = shared_config.shared_data.lock()?;
    let mut certificate_manager = CertificateManager::new(Arc::new(app_config.clone()));
    certificate_manager.start_renewal_task();

    for (_, item) in app_config.api_service_config.iter_mut() {
        let port = item.listen_port;
        let server_type = item.server_type.clone();
        let mapping_key = format!("{port}-{server_type}");
        let (sender, receiver) = mpsc::channel::<()>(1000);
        item.sender = sender;
        let cloned_config = shared_config.clone();

        tokio::task::spawn(async move {
            if let Err(err) =
                start_proxy(cloned_config, port, receiver, server_type, mapping_key).await
            {
                error!("{err}");
            }
        });
    }
    Ok(())
}

pub async fn start_proxy(
    shared_config: SharedConfig,
    port: i32,
    channel: mpsc::Receiver<()>,
    server_type: ServiceType,
    mapping_key: String,
) -> Result<(), AppError> {
    if server_type == ServiceType::Http {
        let mut http_proxy = HttpProxy {
            shared_config,
            port,
            channel,
            mapping_key: mapping_key.clone(),
        };
        http_proxy.start_http_server().await
    } else if server_type == ServiceType::Https {
        let mut http_proxy = HttpProxy {
            shared_config: shared_config.clone(),
            port,
            channel,
            mapping_key: mapping_key.clone(),
        };
        let domains = {
            let config = shared_config.shared_data.lock()?;
            config
                .api_service_config
                .get(&port)
                .ok_or(app_error!(
                    "Missing 'domains' configuration for HTTPS service on port {}",
                    port
                ))?
                .domain_config
                .to_vec()
        };
        http_proxy.start_https_server(domains).await
    } else if server_type == ServiceType::Tcp {
        let mut tcp_proxy = TcpProxy {
            shared_config,
            port,
            mapping_key,
            channel,
        };
        tcp_proxy.start_proxy().await
    } else if server_type == ServiceType::Http2 {
        let mut grpc_proxy = GrpcProxy {
            shared_config,
            port,
            mapping_key,
            channel,
        };
        grpc_proxy.start_proxy().await
    } else {
        let mut grpc_proxy = GrpcProxy {
            shared_config: shared_config.clone(),
            port,
            mapping_key,
            channel,
        };
        let domains = {
            let config = shared_config.shared_data.lock()?;
            config
                .api_service_config
                .get(&port)
                .ok_or(app_error!(
                    "Missing 'domains' configuration for HTTPS service on port {}",
                    port
                ))?
                .domain_config
                .to_vec()
        };
        grpc_proxy.start_tls_proxy(domains).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vojo::app_config::{ApiService, AppConfig, ServiceType};
    use crate::vojo::cli::SharedConfig;

    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::sync::mpsc;
    #[tokio::test]
    async fn test_start_proxy_http() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (tx, rx) = mpsc::channel(1);

        let proxy_task = tokio::spawn(start_proxy(
            shared_config,
            8080,
            rx,
            ServiceType::Http,
            "test-http".to_string(),
        ));

        tokio::time::sleep(Duration::from_millis(10)).await; // Give it time to start
        let res = tx.send(()).await;
        assert!(res.is_ok(), "Expected Ok, got {res:?}");
        let result = proxy_task.await.expect("Proxy task panicked");
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }
    #[tokio::test]
    async fn test_start_proxy_https_success() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (tx, rx) = mpsc::channel(1);

        let proxy_task = tokio::spawn(start_proxy(
            shared_config,
            8081,
            rx,
            ServiceType::Https,
            "test-https".to_string(),
        ));
        tokio::time::sleep(Duration::from_millis(10)).await;
        let cc = tx.send(()).await;
        let result = proxy_task.await.expect("Proxy task panicked");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_start_proxy_https_missing_cert() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (_tx, rx) = mpsc::channel(1); // tx not used as it should fail before listening

        let result = start_proxy(
            shared_config,
            8082,
            rx,
            ServiceType::Https,
            "test-https-fail".to_string(),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            AppError("Private key (key_str) is missing for TLS service on port 8082".to_string())
        );
    }

    #[tokio::test]
    async fn test_start_proxy_tcp() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (tx, rx) = mpsc::channel(1);

        let proxy_task = tokio::spawn(start_proxy(
            shared_config,
            8083,
            rx,
            ServiceType::Tcp,
            "test-tcp".to_string(),
        ));
        tokio::time::sleep(Duration::from_millis(10)).await;
        tx.send(()).await.expect("Failed to send shutdown signal");
        let result = proxy_task.await.expect("Proxy task panicked");
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }

    #[tokio::test]
    async fn test_start_proxy_http2() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (tx, rx) = mpsc::channel(1);

        let proxy_task = tokio::spawn(start_proxy(
            shared_config,
            8084,
            rx,
            ServiceType::Http2,
            "test-http2".to_string(),
        ));
        tokio::time::sleep(Duration::from_millis(10)).await;
        tx.send(()).await.expect("Failed to send shutdown signal");
        let result = proxy_task.await.expect("Proxy task panicked");
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }

    #[tokio::test]
    async fn test_start_proxy_grpc_tls_success() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (tx, rx) = mpsc::channel(1);

        let proxy_task = tokio::spawn(start_proxy(
            shared_config,
            8085,
            rx,
            ServiceType::Http2Tls,
            "test-grpc-tls".to_string(),
        ));
        tokio::time::sleep(Duration::from_millis(10)).await;
        let tt = tx.send(()).await;
        println!("{tt:?}");
        let result = proxy_task.await.expect("Proxy task panicked");
        assert!(result.is_err(), "Expected Ok, got {result:?}");
    }

    #[tokio::test]
    async fn test_start_proxy_grpc_tls_missing_key() {
        let shared_config = SharedConfig::from_app_config(AppConfig::default());
        let (_tx, rx) = mpsc::channel(1);

        let result = start_proxy(
            shared_config,
            8086,
            rx,
            ServiceType::Http2Tls,
            "test-grpc-tls-fail".to_string(),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            AppError("Private key (key_str) is missing for TLS service on port 8086".to_string())
        );
    }

    #[tokio::test]
    async fn test_init_function() {
        let services_to_init = vec![("http_service".to_string(), 9001, ServiceType::Http)];
        let shared_config = SharedConfig::from_app_config(AppConfig {
            api_service_config: HashMap::from([(
                9001,
                ApiService {
                    listen_port: 9001,

                    ..Default::default()
                },
            )]),
            ..Default::default()
        });

        let init_result = init(shared_config.clone()).await;
        assert!(init_result.is_ok());
        {
            let app_config_guard = shared_config.shared_data.lock().unwrap();
            for (_, port, service_conf) in &services_to_init {
                let api_service = app_config_guard
                    .api_service_config
                    .get(&9001)
                    .expect("Service not found in config after init");
                assert_eq!(api_service.listen_port, *port);
                assert_eq!(api_service.server_type, service_conf.clone());
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        println!("test_init_function completed.");
    }
}
