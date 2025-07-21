use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TimeoutConfig {
    pub request_timeout: u64,
}
