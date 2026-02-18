use serde::{Deserialize, Serialize};

/// A tag for organizing books.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default)]
    pub book_count: u32,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: None,
            description: None,
            book_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_new() {
        let tag = Tag::new("rust");
        assert_eq!(tag.name, "rust");
        assert_eq!(tag.book_count, 0);
        assert!(tag.color.is_none());
    }
}
