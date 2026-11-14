use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SoloistWsCommand<'a> {
    pub command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SoloistWsResponse {
    #[serde(default)]
    pub device_name: Option<String>,
}
