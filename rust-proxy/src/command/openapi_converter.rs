// src/command/openapi_converter.rs

use crate::vojo::app_config::{ApiService, AppConfig, RouteConfig};
use crate::vojo::app_error::AppError;
use crate::vojo::cli::ConvertArgs;
use crate::vojo::matcher::{MatcherRule, PathMatchType};
use crate::vojo::router::{BaseRoute, RandomRoute, Router};
use std::collections::{HashMap, HashSet};
use url::Url;

pub async fn handle_convert_command(args: ConvertArgs) -> Result<(), AppError> {
    let yaml = std::fs::read_to_string(args.input_file)?;
    let spec = oas3::from_yaml(&yaml).map_err(|e| AppError(e.to_string()))?;

    let mut app_config = AppConfig::default();
    let mut services: HashMap<i32, ApiService> = HashMap::new();

    let default_server_url = "http://127.0.0.1:8080".to_string();
    let servers = if spec.servers.is_empty() {
        vec![default_server_url]
    } else {
        spec.servers.iter().map(|item| item.url.clone()).collect()
    };

    let default_upstream = servers
        .first()
        .cloned()
        .unwrap_or_else(|| "http://localhost:8000".to_string().clone());
    let paths = spec
        .paths
        .ok_or(AppError("No paths found in the OpenAPI spec".to_string()))?;
    for (path, path_item_ref) in paths.iter() {
        let operation = path_item_ref.methods();
        for (method, operation) in operation.into_iter() {
            for server in &servers {
                let url = Url::parse(server)?;
                let port = url.port_or_known_default().unwrap_or(80) as i32;

                let service = services.entry(port).or_insert_with(|| {
                    let (sender, _) = tokio::sync::mpsc::channel(1);
                    ApiService {
                        listen_port: port,
                        server_type: match url.scheme() {
                            "https" => crate::vojo::app_config::ServiceType::Https,
                            _ => crate::vojo::app_config::ServiceType::Http,
                        },
                        sender,
                        ..Default::default()
                    }
                });

                let mut methods = HashSet::new();
                methods.insert(method.as_str().to_string());

                let route_config = RouteConfig {
                    matchers: vec![
                        MatcherRule::Path {
                            value: path.clone(),
                            match_type: PathMatchType::Exact,
                        },
                        MatcherRule::Method { values: methods },
                    ],
                    router: Router::Random(RandomRoute {
                        routes: vec![BaseRoute {
                            endpoint: default_upstream.clone(),
                            ..Default::default()
                        }],
                    }),
                    ..Default::default()
                };
                service.route_configs.push(route_config);
            }
        }
    }

    app_config.api_service_config = services;

    let output_yaml = serde_yaml::to_string(&app_config)
        .map_err(|e| AppError(format!("Failed to serialize to YAML: {e}")))?;

    println!("{output_yaml}");

    Ok(())
}
