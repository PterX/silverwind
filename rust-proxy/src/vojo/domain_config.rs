use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct AutoHttpsConfig {
    #[serde(rename = "domains")]
    pub domain_names: Vec<String>,
}
