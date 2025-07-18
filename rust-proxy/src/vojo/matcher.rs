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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum MatcherRule {
    #[serde(rename = "path")]
    Path {
        value: String,
        #[serde(default)]
        match_type: PathMatchType,
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
    Method {
        values: HashSet<String>,
    },
}
impl PartialEq for MatcherRule {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Path {
                    value: l_val,
                    match_type: l_mt,
                },
                Self::Path {
                    value: r_val,
                    match_type: r_mt,
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
            } => match match_type {
                PathMatchType::Prefix => path.starts_with(value),
                PathMatchType::Exact => path == value,
            },
            MatcherRule::Method { values } => values.contains(method.as_str()),
            MatcherRule::Host { value, regex } => {
                if regex.is_none() {
                    *regex = Regex::new(value).ok();
                }
                if let (Some(host_header), Some(re)) = (headers.get("Host"), regex.as_ref()) {
                    host_header.to_str().is_ok_and(|h| re.is_match(h))
                } else {
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
                if let (Some(header_value), Some(re)) = (headers.get(name), regex.as_ref()) {
                    header_value.to_str().is_ok_and(|h| re.is_match(h))
                } else {
                    false
                }
            }
        }
    }
}
