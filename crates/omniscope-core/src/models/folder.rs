use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub library_id: Option<String>,
    pub disk_path: Option<String>,
}

impl Folder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: name.into(),
            parent_id: None,
            library_id: None,
            disk_path: None,
        }
    }

    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    pub fn with_parent_opt(mut self, parent_id: Option<&str>) -> Self {
        self.parent_id = parent_id.map(|s| s.to_string());
        self
    }

    pub fn with_library(mut self, library_id: impl Into<String>) -> Self {
        self.library_id = Some(library_id.into());
        self
    }

    pub fn with_library_opt(mut self, library_id: Option<&str>) -> Self {
        self.library_id = library_id.map(|s| s.to_string());
        self
    }

    pub fn with_disk_path(mut self, disk_path: impl Into<String>) -> Self {
        self.disk_path = Some(disk_path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_new() {
        let folder = Folder::new("Papers");
        assert_eq!(folder.name, "Papers");
        assert!(folder.parent_id.is_none());
    }

    #[test]
    fn test_folder_builder() {
        let folder = Folder::new("AI Papers")
            .with_parent("parent-123")
            .with_library("library-456")
            .with_disk_path("/papers/ai");

        assert_eq!(folder.name, "AI Papers");
        assert_eq!(folder.parent_id, Some("parent-123".to_string()));
        assert_eq!(folder.library_id, Some("library-456".to_string()));
        assert_eq!(folder.disk_path, Some("/papers/ai".to_string()));
    }
}
