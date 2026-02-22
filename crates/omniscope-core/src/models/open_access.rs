use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenAccessInfo {
    pub is_open: bool,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oa_url: Option<String>,
    
    #[serde(default)]
    pub pdf_urls: Vec<String>,
}
