use serde::{Deserialize, Serialize};

/// A library is a top-level collection (like a collection in Zotero).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(default)]
    pub book_count: u32,
}

impl Library {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let id = name.to_lowercase().replace(' ', "-");
        Self {
            id,
            name,
            description: None,
            icon: None,
            color: None,
            book_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_new() {
        let lib = Library::new("Programming Books");
        assert_eq!(lib.id, "programming-books");
        assert_eq!(lib.name, "Programming Books");
        assert_eq!(lib.book_count, 0);
    }

    #[test]
    fn test_library_json_roundtrip() {
        let lib = Library::new("Test");
        let json = serde_json::to_string(&lib).unwrap();
        let restored: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, lib.id);
        assert_eq!(restored.name, lib.name);
    }
}
