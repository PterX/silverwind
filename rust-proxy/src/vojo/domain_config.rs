use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DomainsConfig {
    #[serde(rename = "domain")]
    pub domain_name: String,
}
