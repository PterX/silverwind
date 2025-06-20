use crate::vojo::app_error::AppError;
use bytes::Bytes;
use prost_reflect::DescriptorPool;
use prost_reflect::DynamicMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::client::Grpc;
use tonic::transport::Channel;
#[derive(Clone)]

pub struct GrpcChanel {
    pub channel: Grpc<Channel>,
    pub descriptor_pool: DescriptorPool,
}
impl GrpcChanel {
    pub fn do_request(
        &self,
        service_name: String,
        method_name: String,
        body: Bytes,
    ) -> Result<(), AppError> {
        let service_descriptor = self
            .descriptor_pool
            .get_service_by_name(service_name.as_str())
            .ok_or_else(|| AppError(format!("Service '{}' not found", service_name)))?;

        let method_descriptor = service_descriptor
            .methods()
            .into_iter()
            .find(|x| x.name() == method_name)
            .ok_or_else(|| {
                AppError(format!(
                    "Method '{}' not found in service '{}'",
                    method_name, service_name
                ))
            })?;

        // 2. 使用输入消息的描述符来解码请求体
        let request_descriptor = method_descriptor.input();
        let dynamic_request = DynamicMessage::decode(request_descriptor, body).unwrap();
        let codec = prost_reflect::prost::codec::DynamicCodec::new(self.descriptor_pool.clone());
        let req = tonic::Request::new(dynamic_request);
        let path = http::uri::PathAndQuery::try_from(format!(
            "/{}/{}",
            service_descriptor.full_name(),
            method_descriptor.name()
        ))?;
        let grpc_response = self
            .channel
            .unary(req, path, tonic::codec::ProstCodec::new())
            .await?;
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct GrpcClients {
    clients: Arc<HashMap<String, GrpcChanel>>,
}

impl GrpcClients {
    pub async fn new(endpoints: Vec<(String, String)>) -> Result<Self, AppError> {
        let mut clients = HashMap::new();

        for (endpoint_item, proto_descriptor_set) in endpoints {
            let endpoint = Channel::from_shared(endpoint_item.clone())?;
            let channel = endpoint.connect().await?;

            let file_bytes = std::fs::read(&proto_descriptor_set).map_err(|e| {
                AppError(format!(
                    "Failed to read .pb file at '{}': {}",
                    proto_descriptor_set, e,
                ))
            })?;

            let descriptor_pool = DescriptorPool::decode(file_bytes.as_ref())?;
            clients.insert(
                endpoint_item,
                GrpcChanel {
                    channel: Grpc::new(channel),
                    descriotor_pool: descriptor_pool,
                },
            );
        }
        Ok(GrpcClients {
            clients: Arc::new(clients),
        })
    }

    pub async fn get_client(&self, endpoint: &str) -> Result<GrpcChanel, AppError> {
        let client = self
            .clients
            .get(endpoint)
            .cloned()
            .ok_or(AppError::from("Can not find chanel."))?;
        Ok(client)
    }
}
