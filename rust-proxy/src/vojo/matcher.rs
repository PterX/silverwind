use http::{HeaderMap, Method};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum PathMatchType {
    #[serde(rename = "prefix")]
    #[default]
    Prefix,
    #[serde(rename = "exact")]
    Exact,
    #[serde(rename = "regex")]
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum MatcherRule {
    #[serde(rename = "path")]
    Path {
        value: String,
        #[serde(default)]
        match_type: PathMatchType,
        #[serde(skip)]
        #[serde(default)]
        regex: Option<Regex>,
    },
    #[serde(rename = "host")]
    Host {
        value: String,
        #[serde(skip)]
        #[serde(default)]
        regex: Option<Regex>,
    },
    #[serde(rename = "header")]
    Header {
        name: String,
        value: String,
        #[serde(skip)]
        #[serde(default)]
        regex: Option<Regex>,
    },
    #[serde(rename = "method")]
    Method { values: HashSet<String> },
}
impl PartialEq for MatcherRule {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Path {
                    value: l_val,
                    match_type: l_mt,
                    ..
                },
                Self::Path {
                    value: r_val,
                    match_type: r_mt,
                    ..
                },
            ) => l_val == r_val && l_mt == r_mt,

            (Self::Host { value: l_val, .. }, Self::Host { value: r_val, .. }) => l_val == r_val,

            (
                Self::Header {
                    name: l_name,
                    value: l_val,
                    ..
                },
                Self::Header {
                    name: r_name,
                    value: r_val,
                    ..
                },
            ) => l_name == r_name && l_val == r_val,
            (Self::Method { values: l_vals }, Self::Method { values: r_vals }) => l_vals == r_vals,
            _ => false,
        }
    }
}
impl MatcherRule {
    pub fn matches(&mut self, method: &Method, path: &str, headers: &HeaderMap) -> bool {
        match self {
            MatcherRule::Path {
                ref value,
                match_type,
                regex,
            } => match match_type {
                PathMatchType::Prefix => {
                    if path.starts_with(value) {
                        true
                    } else {
                        debug!(
                            "Path matching failed: path '{path}' does not start with prefix '{value}'"
                        );
                        false
                    }
                }
                PathMatchType::Exact => {
                    if path == value.as_str() {
                        true
                    } else {
                        debug!(
                            "Path matching failed: path '{path}' does not exactly match '{value}'"
                        );
                        false
                    }
                }
                PathMatchType::Regex => {
                    if regex.is_none() {
                        *regex = Regex::new(value).ok();
                        if regex.is_none() {
                            debug!("Path matching failed: invalid regex pattern '{value}'");
                        }
                    }

                    if let Some(re) = regex.as_ref() {
                        if re.is_match(path) {
                            true
                        } else {
                            debug!(
                                "Path matching failed: path '{path}' does not match regex '{value}'"
                            );
                            false
                        }
                    } else {
                        // 如果编译失败，则匹配失败
                        false
                    }
                }
            },
            MatcherRule::Method { values } => {
                if values.contains(&method.as_str().to_string()) {
                    true
                } else {
                    debug!(
                        "Method matching failed: method '{method}' is not in the allowed list '{values:?}'"
                    );
                    false
                }
            }
            MatcherRule::Host { value, regex } => {
                if regex.is_none() {
                    *regex = Regex::new(value).ok();
                }
                if let (Some(host_header), Some(re)) = (headers.get("Host"), regex.as_ref()) {
                    match host_header.to_str() {
                        Ok(h) => {
                            if re.is_match(h) {
                                true
                            } else {
                                debug!(
                                    "Host matching failed: host '{h}' does not match regex '{value}'"
                                );
                                false
                            }
                        }
                        Err(_) => {
                            debug!("Host matching failed: 'Host' header contains non-visible ASCII characters");
                            false
                        }
                    }
                } else {
                    if headers.get("Host").is_none() {
                        debug!("Host matching failed: 'Host' header not found");
                    }
                    if regex.is_none() {
                        debug!("Host matching failed: invalid regex pattern '{value}'");
                    }
                    false
                }
            }
            MatcherRule::Header {
                ref name,
                value,
                regex,
            } => {
                if regex.is_none() {
                    *regex = Regex::new(value).ok();
                }
                if let (Some(header_value), Some(re)) = (headers.get(name.as_str()), regex.as_ref())
                {
                    match header_value.to_str() {
                        Ok(h) => {
                            if re.is_match(h) {
                                true
                            } else {
                                debug!(
                                    "Header matching failed: header '{name}' with value '{h}' does not match regex '{value}'"
                                );
                                false
                            }
                        }
                        Err(_) => {
                            debug!(
                                "Header matching failed: header '{name}' contains non-visible ASCII characters"
                            );
                            false
                        }
                    }
                } else {
                    if headers.get(name.as_str()).is_none() {
                        debug!("Header matching failed: header '{name}' not found");
                    }
                    if regex.is_none() {
                        debug!(
                            "Header matching failed: invalid regex pattern '{value}' for header '{name}'"
                        );
                    }
                    false
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use http::{header, HeaderValue, Method};
    use std::collections::HashSet;

    #[test]
    fn test_path_prefix_match_success() {
        let mut rule = MatcherRule::Path {
            value: "/api/v1".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(rule.matches(&Method::GET, "/api/v1/users", &headers));
    }

    #[test]
    fn test_path_prefix_match_failure() {
        let mut rule = MatcherRule::Path {
            value: "/api/v1".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/app/v1/users", &headers));
    }

    #[test]
    fn test_path_exact_match_success() {
        let mut rule = MatcherRule::Path {
            value: "/api/v1/users".to_string(),
            match_type: PathMatchType::Exact,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(rule.matches(&Method::GET, "/api/v1/users", &headers));
    }

    #[test]
    fn test_path_exact_match_failure() {
        let mut rule = MatcherRule::Path {
            value: "/api/v1/users".to_string(),
            match_type: PathMatchType::Exact,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/api/v1/users/1", &headers));
    }

    #[test]
    fn test_path_regex_match_success() {
        let mut rule = MatcherRule::Path {
            value: "^/users/\\d+$".to_string(),
            match_type: PathMatchType::Regex,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(rule.matches(&Method::GET, "/users/123", &headers));
    }

    #[test]
    fn test_path_regex_match_failure() {
        let mut rule = MatcherRule::Path {
            value: "^/users/\\d+$".to_string(),
            match_type: PathMatchType::Regex,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/users/abc", &headers));
    }

    #[test]
    fn test_path_regex_invalid_pattern() {
        let mut rule = MatcherRule::Path {
            value: "[".to_string(), // Invalid regex pattern
            match_type: PathMatchType::Regex,
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/users/123", &headers));
    }

    #[test]
    fn test_method_match_success() {
        let mut rule = MatcherRule::Method {
            values: HashSet::from(["GET".to_string(), "POST".to_string()]),
        };
        let headers = HeaderMap::new();
        assert!(rule.matches(&Method::GET, "/any/path", &headers));
    }

    #[test]
    fn test_method_match_failure() {
        let mut rule = MatcherRule::Method {
            values: HashSet::from(["GET".to_string(), "POST".to_string()]),
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::PUT, "/any/path", &headers));
    }

    #[test]
    fn test_host_match_success() {
        let mut rule = MatcherRule::Host {
            value: r"^(api|www)\.example\.com$".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        assert!(rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_host_match_failure() {
        let mut rule = MatcherRule::Host {
            value: r"^(api|www)\.example\.com$".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("other.example.com"));
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_host_match_header_missing() {
        let mut rule = MatcherRule::Host {
            value: r"^(api|www)\.example\.com$".to_string(),
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_host_match_invalid_regex() {
        let mut rule = MatcherRule::Host {
            value: "[invalid".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_header_match_success() {
        let mut rule = MatcherRule::Header {
            name: "x-request-id".to_string(),
            value: r"^token-\d+$".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("token-12345"));
        assert!(rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_header_match_failure_value() {
        let mut rule = MatcherRule::Header {
            name: "x-request-id".to_string(),
            value: r"^token-\d+$".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-request-id",
            HeaderValue::from_static("invalid-token-format"),
        );
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_header_match_failure_missing() {
        let mut rule = MatcherRule::Header {
            name: "x-request-id".to_string(),
            value: r"^token-\d+$".to_string(),
            regex: None,
        };
        let headers = HeaderMap::new();
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_header_match_invalid_regex() {
        let mut rule = MatcherRule::Header {
            name: "x-request-id".to_string(),
            value: "[invalid".to_string(),
            regex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("token-12345"));
        assert!(!rule.matches(&Method::GET, "/", &headers));
    }

    #[test]
    fn test_partialeq_path_equal() {
        let r1 = MatcherRule::Path {
            value: "/path".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        let r2 = MatcherRule::Path {
            value: "/path".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_partialeq_path_not_equal() {
        let r1 = MatcherRule::Path {
            value: "/path".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        let r2 = MatcherRule::Path {
            value: "/path".to_string(),
            match_type: PathMatchType::Exact,
            regex: None,
        };
        assert_ne!(r1, r2);
    }

    #[test]
    fn test_partialeq_different_variants() {
        let r1 = MatcherRule::Path {
            value: "/path".to_string(),
            match_type: PathMatchType::Prefix,
            regex: None,
        };
        let r2 = MatcherRule::Host {
            value: "example.com".to_string(),
            regex: None,
        };
        assert_ne!(r1, r2);
    }

    #[test]
    fn test_partialeq_ignores_cached_regex() {
        let mut r1 = MatcherRule::Path {
            value: "^/users/\\d+$".to_string(),
            match_type: PathMatchType::Regex,
            regex: None,
        };
        let r2 = MatcherRule::Path {
            value: "^/users/\\d+$".to_string(),
            match_type: PathMatchType::Regex,
            regex: None,
        };

        assert_eq!(r1, r2);

        let headers = HeaderMap::new();
        r1.matches(&Method::GET, "/users/123", &headers);

        assert!(matches!(&r1, MatcherRule::Path { regex, .. } if regex.is_some()));
        assert!(matches!(&r2, MatcherRule::Path { regex, .. } if regex.is_none()));

        assert_eq!(r1, r2);
    }
}
