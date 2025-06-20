use crate::middleware::middlewares::Middleware;
use crate::AppError;
use bytes::Bytes;
use http::header::{HeaderName, HeaderValue};
use http::Request;
use http_body_util::combinators::BoxBody;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RequestHeaders {
    #[serde(default)]
    pub add: HashMap<String, String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

impl Middleware for RequestHeaders {
    fn handle_request(
        &mut self,
        _peer_addr: SocketAddr,
        req: &mut Request<BoxBody<Bytes, AppError>>,
    ) -> Result<(), AppError> {
        let headers = req.headers_mut();

        for key in &self.remove {
            if let Ok(header_name) = HeaderName::from_str(key) {
                headers.remove(header_name);
            }
        }
        for (key, value) in &self.add {
            let header_name = HeaderName::from_str(key)
                .map_err(|e| AppError(format!("Invalid header name '{}': {}", key, e)))?;

            let header_value = HeaderValue::from_str(value)
                .map_err(|e| AppError(format!("Invalid header value for '{}': {}", key, e)))?;
            debug!("Adding header: {}: {}", key, value);
            headers.insert(header_name, header_value);
        }
        Ok(())
    }
}
