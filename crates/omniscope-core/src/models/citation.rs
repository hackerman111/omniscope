use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookCitationGraph {
    #[serde(default)]
    pub citation_count: u32,

    #[serde(default)]
    pub cited_by_ids: Vec<Uuid>,

    #[serde(default)]
    pub references_ids: Vec<Uuid>,
}
