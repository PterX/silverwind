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
