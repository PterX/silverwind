use crate::middleware::middlewares::CheckResult;
use crate::middleware::middlewares::MiddleWares;
use crate::middleware::middlewares::Middleware;
use crate::utils::uuid::get_uuid;
use crate::vojo::anomaly_detection::AnomalyDetectionType;
use crate::vojo::app_error::AppError;
use crate::vojo::health_check::HealthCheckType;
use crate::vojo::matcher::MatcherRule;
use crate::vojo::router::deserialize_router;
use crate::vojo::router::Router;
use crate::vojo::timeout_config::TimeoutConfig;
use crate::DEFAULT_ADMIN_PORT;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use regex::Regex;
use serde::Deserializer;
use serde::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing_subscriber::filter::LevelFilter;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_log_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,
    #[serde(
        rename = "servers",
        deserialize_with = "deserialize_service_config",
        serialize_with = "serialize_api_service_config"
    )]
    pub api_service_config: HashMap<i32, ApiService>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            health_check_log_enabled: Some(false),
            admin_port: Some(DEFAULT_ADMIN_PORT),
            log_level: None,
            api_service_config: Default::default(),
        }
    }
}
fn serialize_api_service_config<S>(
    config: &HashMap<i32, ApiService>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let vec: Vec<ApiService> = config.values().cloned().collect();
    vec.serialize(serializer)
}

fn deserialize_service_config<'de, D>(deserializer: D) -> Result<HashMap<i32, ApiService>, D::Error>
where
    D: Deserializer<'de>,
{
    let services: Vec<ApiService> = Deserialize::deserialize(deserializer)?;

    let mut hashmap = HashMap::new();
    for item in services {
        hashmap.insert(item.listen_port, item);
    }
    Ok(hashmap)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Matcher {
    pub prefix: String,
    pub prefix_rewrite: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LivenessConfig {
    pub min_liveness_count: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LivenessStatus {
    pub current_liveness_count: i32,
}
fn is_empty(value: &str) -> bool {
    value.is_empty()
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcode {
    pub proto_descriptor_set: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RouteConfig {
    #[serde(skip_serializing_if = "is_empty", default = "default_route_id")]
    pub route_id: String,

    #[serde(default)]
    pub matchers: Vec<MatcherRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_rewrite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcode: Option<Transcode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_detection: Option<AnomalyDetectionType>,
    #[serde(skip_deserializing, skip_serializing)]
    pub liveness_status: LivenessStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liveness_config: Option<LivenessConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckType>,
    #[serde(deserialize_with = "deserialize_router", rename = "forward_to")]
    pub router: Router,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middlewares: Option<Vec<MiddleWares>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<TimeoutConfig>,
}

fn default_route_id() -> String {
    get_uuid()
}

impl RouteConfig {
    pub fn match_and_rewrite(
        &mut self,
        path: &str,
        method: &Method,
        headers: &HeaderMap<HeaderValue>,
    ) -> Result<Option<String>, AppError> {
        if self
            .matchers
            .iter_mut()
            .all(|rule| rule.matches(method, path, headers))
        {
        } else {
            return Ok(None);
        }
        if let Some(rewrite_template) = &self.path_rewrite {
            let path_pattern = self.matchers.iter().find_map(|m| match m {
                MatcherRule::Path { value, .. } => Some(value),
                _ => None,
            });
            if let Some(pattern) = path_pattern {
                if let Ok(re) = Regex::new(pattern) {
                    let rewritten_path = re.replace(path, rewrite_template.as_str());
                    return Ok(Some(rewritten_path.into_owned()));
                }

                if let Some(stripped_path) = path.strip_prefix(pattern) {
                    return Ok(Some(format!("{rewrite_template}{stripped_path}")));
                }
            }

            Ok(Some(path.to_string()))
        } else {
            Ok(Some(path.to_string()))
        }
    }

    pub fn is_allowed(
        &mut self,
        peer_addr: &SocketAddr,
        headers_option: Option<&HeaderMap<HeaderValue>>,
    ) -> Result<CheckResult, AppError> {
        if let Some(middlewares) = &mut self.middlewares {
            for middleware in middlewares.iter_mut() {
                let check_res = middleware.check_request(peer_addr, headers_option)?;
                if !check_res.is_allowed() {
                    return Ok(check_res);
                }
            }
        }
        Ok(CheckResult::Allowed)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, strum_macros::Display)]
pub enum ServiceType {
    #[default]
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "https")]
    Https,
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "tls")]
    Http2,
    Http2Tls,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiService {
    #[serde(rename = "listen")]
    pub listen_port: i32,

    #[serde(rename = "protocol")]
    pub server_type: ServiceType,
    #[serde(rename = "domains")]
    pub domain_config: Vec<String>,
    #[serde(rename = "routes")]
    pub route_configs: Vec<RouteConfig>,
    #[serde(skip_deserializing, skip_serializing)]
    pub sender: mpsc::Sender<()>,
}
impl<'de> Deserialize<'de> for ApiService {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ApiServiceWithoutSender {
            #[serde(rename = "listen")]
            port: i32,
            #[serde(rename = "protocol")]
            pub server_type: ServiceType,
            #[serde(rename = "domains")]
            #[serde(default)]
            pub domain_config: Vec<String>,
            #[serde(rename = "routes")]
            pub route_configs: Vec<RouteConfig>,
        }

        let api_service_without_sender = ApiServiceWithoutSender::deserialize(deserializer)?;
        let (sender, _) = mpsc::channel(1);

        Ok(ApiService {
            listen_port: api_service_without_sender.port,
            server_type: api_service_without_sender.server_type,
            domain_config: api_service_without_sender.domain_config,
            route_configs: api_service_without_sender.route_configs,
            sender,
        })
    }
}
impl PartialEq for ApiService {
    fn eq(&self, other: &Self) -> bool {
        self.listen_port == other.listen_port
            && self.server_type == other.server_type
            && self.route_configs == other.route_configs
    }
}
impl Default for ApiService {
    fn default() -> Self {
        let (sender, _) = mpsc::channel(1); // Buffer size 1

        Self {
            listen_port: Default::default(),
            server_type: Default::default(),
            domain_config: Default::default(),
            route_configs: Default::default(),

            sender,
        }
    }
}

impl AppConfig {
    pub fn get_log_level(&self) -> LevelFilter {
        match self.log_level {
            Some(LogLevel::Debug) => LevelFilter::DEBUG,
            Some(LogLevel::Info) => LevelFilter::INFO,
            Some(LogLevel::Error) => LevelFilter::ERROR,
            Some(LogLevel::Warn) => LevelFilter::WARN,
            Some(LogLevel::Trace) => LevelFilter::TRACE,

            None => LevelFilter::OFF,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "trace")]
    Trace,
}

#[cfg(test)]
mod tests {

    use std::time::SystemTime;

    use super::*;
    use crate::middleware::authentication::ApiKeyAuth;

    use crate::middleware::cors_config::CorsAllowedOrigins;

    use crate::middleware::rate_limit::IPBasedRatelimit;
    use crate::middleware::rate_limit::TimeUnit;
    use crate::middleware::rate_limit::TokenBucketRateLimit;
    use crate::vojo::health_check::BaseHealthCheckParam;
    use crate::vojo::health_check::HttpHealthCheckParam;
    use crate::vojo::router::BaseRoute;
    use crate::vojo::router::WeightedRouteItem;

    use crate::middleware::allow_deny_ip::AllowType;
    use crate::middleware::allow_deny_ip::{AllowDenyIp, AllowDenyItem};
    use crate::middleware::authentication::Authentication;
    use crate::middleware::cors_config::CorsAllowHeader;
    use crate::middleware::cors_config::CorsConfig;
    use crate::middleware::cors_config::Method;
    use crate::middleware::rate_limit::LimitLocation;
    use crate::middleware::rate_limit::Ratelimit;
    use crate::vojo::router::HeaderBasedRoute;
    use crate::vojo::router::HeaderRoutingRule;
    use crate::vojo::router::PollRoute;
    use crate::vojo::router::RandomRoute;
    use std::sync::{Arc, Mutex};

    use crate::vojo::router::WeightBasedRoute;
    use http::HeaderValue;
    fn create_api_service1() -> ApiService {
        let mut api_service = ApiService {
            listen_port: 8081,
            ..Default::default()
        };

        let header_based = WeightBasedRoute {
            routes: vec![
                WeightedRouteItem {
                    weight: 1,
                    index: 0,
                    endpoint: "http://127.0.0.1:9394".to_string(),
                    ..Default::default()
                },
                WeightedRouteItem {
                    weight: 2,
                    index: 0,
                    endpoint: "http://127.0.0.1:9396".to_string(),
                    ..Default::default()
                },
                WeightedRouteItem {
                    weight: 3,
                    index: 0,
                    endpoint: "http://127.0.0.1:9395".to_string(),
                    ..Default::default()
                },
            ],
        };
        let route = RouteConfig {
            route_id: "test_route".to_string(),
            router: Router::WeightBased(header_based),
            ..Default::default()
        };

        api_service.route_configs = vec![route];
        api_service
    }
    fn create_api_service2() -> ApiService {
        let mut api_service = ApiService {
            listen_port: 8082,
            ..Default::default()
        };

        let poll_route = PollRoute {
            routes: vec![
                BaseRoute {
                    endpoint: "http://127.0.0.1:9394".to_string(),
                    ..Default::default()
                },
                BaseRoute {
                    endpoint: "http://127.0.0.1:9395".to_string(),
                    ..Default::default()
                },
                BaseRoute {
                    endpoint: "http://127.0.0.1:9396".to_string(),
                    ..Default::default()
                },
            ],
            current_index: 0,
        };
        let route = RouteConfig {
            route_id: "test_route".to_string(),
            router: Router::Poll(poll_route),

            ..Default::default()
        };

        api_service.route_configs = vec![route];
        api_service
    }
    fn create_api_service3() -> ApiService {
        let mut api_service = ApiService {
            listen_port: 8083,
            ..Default::default()
        };

        let poll_route = RandomRoute {
            routes: vec![
                BaseRoute {
                    endpoint: "http://127.0.0.1:9394".to_string(),
                    ..Default::default()
                },
                BaseRoute {
                    endpoint: "http://127.0.0.1:9395".to_string(),
                    ..Default::default()
                },
                BaseRoute {
                    endpoint: "http://127.0.0.1:9396".to_string(),
                    ..Default::default()
                },
            ],
        };
        let route = RouteConfig {
            route_id: "test_route".to_string(),
            router: Router::Random(poll_route),

            ..Default::default()
        };

        api_service.route_configs = vec![route];
        api_service
    }
    fn create_api_service4() -> ApiService {
        let mut api_service = ApiService {
            listen_port: 8084,
            ..Default::default()
        };

        let poll_route =
            HeaderBasedRoute {
                routes: vec![
                    HeaderRoutingRule {
                        header_key: "test".to_string(),
                        header_value_mapping_type:
                            crate::vojo::router::HeaderValueMappingType::Text("test".to_string()),
                        endpoint: "http://127.0.0.1:9395".to_string(),
                        ..Default::default()
                    },
                    HeaderRoutingRule {
                        header_key: "test".to_string(),
                        header_value_mapping_type:
                            crate::vojo::router::HeaderValueMappingType::Text("test".to_string()),
                        endpoint: "http://127.0.0.1:9396".to_string(),
                        ..Default::default()
                    },
                    HeaderRoutingRule {
                        header_key: "test".to_string(),
                        header_value_mapping_type:
                            crate::vojo::router::HeaderValueMappingType::Text("test".to_string()),
                        endpoint: "http://127.0.0.1:9397".to_string(),
                        ..Default::default()
                    },
                ],
            };
        let route = RouteConfig {
            route_id: "test_route".to_string(),
            router: Router::HeaderBased(poll_route),
            ..Default::default()
        };

        api_service.route_configs = vec![route];
        api_service
    }
    pub fn create_default_app_config() -> AppConfig {
        let mut app_config = AppConfig::default();

        let mut api_service = ApiService {
            listen_port: 8085,
            ..Default::default()
        };

        let header_based = WeightBasedRoute {
            routes: vec![WeightedRouteItem {
                weight: 1,
                index: 0,
                endpoint: "http://127.0.0.1:9393".to_string(),
                ..Default::default()
            }],
        };
        let route = RouteConfig {
            route_id: "test_route".to_string(),
            matchers: vec![MatcherRule::Path {
                value: "/".to_string(),
                match_type: crate::vojo::matcher::PathMatchType::Exact,
                regex: None,
            }],
            router: Router::WeightBased(header_based),

            health_check: Some(HealthCheckType::HttpGet(HttpHealthCheckParam {
                path: "/health".to_string(),
                base_health_check_param: BaseHealthCheckParam {
                    interval: 5,
                    timeout: 5,
                },
            })),

            liveness_config: Some(LivenessConfig {
                min_liveness_count: 1,
            }),
            middlewares: Some(vec![
                MiddleWares::Authentication(Authentication::ApiKey(ApiKeyAuth {
                    key: "test".to_string(),
                    value: "test".to_string(),
                })),
                MiddleWares::RateLimit(Arc::new(Mutex::new(Ratelimit::TokenBucket(
                    TokenBucketRateLimit {
                        capacity: 10,
                        rate_per_unit: 10,
                        scope: LimitLocation::IP(IPBasedRatelimit {
                            value: "192.168.0.1".to_string(),
                        }),
                        unit: TimeUnit::Second,
                        current_count: 10,
                        last_update_time: SystemTime::now(),
                    },
                )))),
                MiddleWares::AllowDenyList(AllowDenyIp {
                    rules: vec![AllowDenyItem {
                        value: Some("192.168.0.2".to_string()),
                        policy: AllowType::AllowAll,
                    }],
                }),
                MiddleWares::Cors(CorsConfig {
                    allow_credentials: Some(true),
                    allowed_origins: CorsAllowedOrigins::All,
                    allowed_methods: vec![Method::Get, Method::Post],
                    allowed_headers: Some(CorsAllowHeader::All),

                    max_age: Some(3600),
                    options_passthrough: Some(true),
                }),
            ]),
            ..Default::default()
        };

        api_service.route_configs = vec![route];
        app_config.api_service_config.insert(8079, api_service);

        app_config
            .api_service_config
            .insert(8080, create_api_service1());
        app_config
            .api_service_config
            .insert(8081, create_api_service2());
        app_config
            .api_service_config
            .insert(8082, create_api_service3());
        app_config
            .api_service_config
            .insert(8083, create_api_service4());
        app_config
    }
    #[test]
    fn test_app_config_serialize_with_static_config() {
        let app_config = create_default_app_config();
        let json_str = serde_yaml::to_string(&app_config).unwrap();
        println!("{json_str}");
    }
    use crate::DEFAULT_ADMIN_PORT;
    #[test]
    fn test_static_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.health_check_log_enabled, Some(false));
        assert_eq!(config.admin_port, Some(DEFAULT_ADMIN_PORT));
        assert_eq!(config.log_level, None);
    }

    #[test]
    fn test_route_matching() {
        let mut route = RouteConfig {
            matchers: vec![MatcherRule::Path {
                value: "/api/test".to_string(),
                match_type: crate::vojo::matcher::PathMatchType::Exact,
                regex: None,
            }],
            path_rewrite: Some("/v1/test".to_string()),
            ..Default::default()
        };
        let get = http::Method::GET;
        let result = route
            .match_and_rewrite("/api/test", &get, &HeaderMap::new())
            .unwrap();
        assert_eq!(result, Some("/v1/test".to_string()));

        let result = route
            .match_and_rewrite("/other/test", &get, &HeaderMap::new())
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_route_host_matching() {
        let mut route = RouteConfig {
            matchers: vec![
                MatcherRule::Path {
                    value: "/api/test".to_string(),
                    match_type: crate::vojo::matcher::PathMatchType::Exact,
                    regex: None,
                },
                MatcherRule::Host {
                    value: "example.com".to_string(),
                    regex: None,
                },
            ],
            path_rewrite: Some("/v1/test".to_string()),

            ..Default::default()
        };

        let mut headers = HeaderMap::new();
        headers.insert("Host", HeaderValue::from_static("example.com"));

        let result = route
            .match_and_rewrite("/api/test", &http::Method::GET, &headers)
            .unwrap();
        assert_eq!(result, Some("/v1/test".to_string()));

        headers.insert("Host", HeaderValue::from_static("wrong.com"));
        let result = route
            .match_and_rewrite("/api/test", &http::Method::GET, &headers)
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_api_service_equality() {
        let service1 = ApiService {
            listen_port: 8080,
            ..Default::default()
        };

        let service2 = ApiService {
            listen_port: 8080,
            ..Default::default()
        };

        assert_eq!(service1, service2);
    }

    #[test]
    fn test_service_config_serialization() {
        let route = RouteConfig {
            router: Router::WeightBased(WeightBasedRoute {
                routes: vec![WeightedRouteItem {
                    weight: 1,
                    index: 0,
                    endpoint: "http://example.com".to_string(),
                    ..Default::default()
                }],
            }),
            ..Default::default()
        };

        let api_service = ApiService {
            listen_port: 8080,
            route_configs: vec![route],
            ..Default::default()
        };

        let mut app_config = AppConfig::default();
        app_config.api_service_config.insert(8080, api_service);

        let yaml = serde_yaml::to_string(&app_config).unwrap();
        assert!(yaml.contains("listen: 8080"));
        assert!(yaml.contains("protocol: http"));
    }

    use crate::vojo::base_response::BaseResponse;
    #[tokio::test]
    async fn test_health_check() {
        let src = r#"
response_code: 0
response_object:
  servers:
  - listen: 8080
    protocol: http
    routes:
      - route_id: route1
        forward_to:
          kind: poll
          targets: []"#;
        let _: BaseResponse<AppConfig> = serde_yaml::from_str(src).unwrap();
    }
}
