use crate::proxy::http1::grpc_client::GrpcClients;
use crate::proxy::http1::http_client::HttpClients;
use crate::vojo::cli::SharedConfig;
use crate::AppError;
#[derive(Clone)]
pub struct AppClients {
    pub http: HttpClients,
    pub grpc: Option<GrpcClients>,
}
impl AppClients {
    pub async fn new(shared_config: SharedConfig, port: i32) -> Result<AppClients, AppError> {
        let http_client = HttpClients::new();
        let grpc_client_result: Result<Option<GrpcClients>, AppError> = async {
            let grpc_list_to_process = {
                let lock = shared_config.shared_data.lock()?;    
                let Some(api_service) = lock.api_service_config.get(&port) else {
                    return Ok(None);
                };
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
                                .ok_or_else(|| AppError::from("Transcode config is required for gRPC routes but was not found."))?
                                .proto_descriptor_set,
                        ));
                    }
                }    
                if grpc_list.is_empty() {
                    Ok::<_, AppError>(None)
                } else {
                    Ok::<_, AppError>(Some(grpc_list))
                }
            }?; 
                if let Some(grpc_list) = grpc_list_to_process {
                match GrpcClients::new(grpc_list).await {
                    Ok(clients) => Ok(Some(clients)),
                    Err(e) => {
                        eprintln!("Failed to initialize gRPC clients for port {port}, proceeding without them. Error: {e}");
                        Ok(None) 
                    }
                }
            } else {
                Ok(None)
            }
        }.await;
        match grpc_client_result {
            Ok(grpc_option) => Ok(AppClients {
                http: http_client,
                grpc: grpc_option,
            }),
            Err(e) => Err(e),
        }
    }
}
