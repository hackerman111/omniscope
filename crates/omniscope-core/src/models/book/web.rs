use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookWeb {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openlibrary_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goodreads_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,

    #[serde(default)]
    pub sources: Vec<WebSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSource {
    pub name: String,
    pub url: String,
}
