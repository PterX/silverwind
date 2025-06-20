use crate::middleware::middlewares::CheckResult;
use crate::middleware::middlewares::Denial;
use crate::middleware::middlewares::Middleware;
use base64::{engine::general_purpose, Engine as _};
use core::fmt::Debug;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::vojo::app_error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "PascalCase")]
pub enum Authentication {
    #[serde(rename = "basic")]
    Basic(BasicAuth),
    #[serde(rename = "api_key")]
    ApiKey(ApiKeyAuth),
}
impl Middleware for Authentication {
    fn check_request(
        &mut self,
        _peer_addr: &SocketAddr,
        headers_option: Option<&HeaderMap<HeaderValue>>,
    ) -> Result<CheckResult, AppError> {
        if let Some(header_map) = headers_option {
            if !self.check_authentication(header_map)? {
                let denial = Denial {
                    status: StatusCode::UNAUTHORIZED,
                    headers: HeaderMap::new(),
                    body: "Authentication failed".to_string(),
                };
                return Ok(CheckResult::Denied(denial));
            }
        }
        Ok(CheckResult::Allowed)
    }
}
impl Authentication {
    pub fn check_authentication(
        &mut self,
        headers: &HeaderMap<HeaderValue>,
    ) -> Result<bool, AppError> {
        match self {
            Authentication::Basic(auth) => auth.check_authentication(headers),
            Authentication::ApiKey(auth) => auth.check_authentication(headers),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BasicAuth {
    pub credentials: String,
}

impl BasicAuth {
    fn check_authentication(&mut self, headers: &HeaderMap<HeaderValue>) -> Result<bool, AppError> {
        if headers.is_empty() || !headers.contains_key("Authorization") {
            return Ok(false);
        }
        let value = headers
            .get("Authorization")
            .ok_or("Can not find Authorization")?
            .to_str()?;
        let split_list: Vec<_> = value.split(' ').collect();
        if split_list.len() != 2 || split_list[0] != "Basic" {
            return Ok(false);
        }
        let encoded: String = general_purpose::STANDARD_NO_PAD.encode(&self.credentials);
        Ok(split_list[1] == encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ApiKeyAuth {
    pub key: String,
    pub value: String,
}

impl ApiKeyAuth {
    fn check_authentication(&mut self, headers: &HeaderMap<HeaderValue>) -> Result<bool, AppError> {
        if headers.is_empty() || !headers.contains_key(&self.key) {
            return Ok(false);
        }
        let header_value = headers.get(&self.key).ok_or("Can not find key")?.to_str()?;
        Ok(header_value == self.value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn test_basic_auth_success() {
        let mut auth = BasicAuth {
            credentials: "user:pass".to_string(),
        };
        let encoded = general_purpose::STANDARD_NO_PAD.encode("user:pass");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap(),
        );

        assert!(auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_basic_auth_missing_header() {
        let mut auth = BasicAuth {
            credentials: "user:pass".to_string(),
        };
        let headers = HeaderMap::new();

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_basic_auth_invalid_format() {
        let mut auth = BasicAuth {
            credentials: "user:pass".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer token"));

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_basic_auth_wrong_credentials() {
        let mut auth = BasicAuth {
            credentials: "user:wrong".to_string(),
        };
        let encoded = general_purpose::STANDARD_NO_PAD.encode("user:pass");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap(),
        );

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_api_key_auth_success() {
        let mut auth = ApiKeyAuth {
            key: "X-API-KEY".to_string(),
            value: "secret".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert("X-API-KEY", HeaderValue::from_static("secret"));

        assert!(auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_api_key_auth_missing_header() {
        let mut auth = ApiKeyAuth {
            key: "X-API-KEY".to_string(),
            value: "secret".to_string(),
        };
        let headers = HeaderMap::new();

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_api_key_auth_wrong_value() {
        let mut auth = ApiKeyAuth {
            key: "X-API-KEY".to_string(),
            value: "secret".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert("X-API-KEY", HeaderValue::from_static("wrong"));

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_api_key_auth_case_sensitive() {
        let mut auth = ApiKeyAuth {
            key: "X-API-KEY".to_string(),
            value: "Secret".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert("X-API-KEY", HeaderValue::from_static("secret"));

        assert!(!auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_authentication_enum_basic() {
        let mut auth = Authentication::Basic(BasicAuth {
            credentials: "admin:admin".to_string(),
        });
        let encoded = general_purpose::STANDARD_NO_PAD.encode("admin:admin");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap(),
        );

        assert!(auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_authentication_enum_api_key() {
        let mut auth = Authentication::ApiKey(ApiKeyAuth {
            key: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        });
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer token"));

        assert!(auth.check_authentication(&headers).unwrap());
    }

    #[test]
    fn test_invalid_header_value() {
        let mut auth = BasicAuth {
            credentials: "user:pass".to_string(),
        };
        let mut headers = HeaderMap::new();
        let invalid_value = vec![0xff, 0xff, 0xff];
        headers.insert(
            "Authorization",
            HeaderValue::from_bytes(&invalid_value).unwrap(),
        );

        let result = auth.check_authentication(&headers);
        assert!(result.is_err());
    }
}
