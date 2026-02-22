use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FolderType {
    Physical,      // Директория на диске
    Virtual,       // Только в метаданных
    LibraryRoot,   // Корень библиотеки
}

impl Default for FolderType {
    fn default() -> Self {
        Self::Physical
    }
}

/// Статус присутствия файла книги на диске
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FilePresence {
    /// Файл есть, путь актуален
    Present { path: PathBuf, size_bytes: u64, hash: Option<String> },
    /// Файл никогда не добавлялся (ghost book)
    NeverHadFile,
    /// Файл был, но исчез (detached book)
    Missing { last_known_path: PathBuf, last_seen: DateTime<Utc> },
}

impl Default for FilePresence {
    fn default() -> Self {
        Self::NeverHadFile
    }
}

// TODO: RelativePath and AbsolutePath need clear definition, for now falling back to String & PathBuf depending on context

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub folder_type: FolderType,
    pub parent_id: Option<String>,
    pub library_id: Option<String>,

    /// Только для PhysicalFolder: реальный путь на диске
    /// Относительный от корня библиотеки: "programming/rust"
    pub disk_path: Option<String>,

    /// Для VirtualFolder: иконка и цвет (пользовательские)
    pub icon: Option<String>,
    pub color: Option<String>,

    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Folder {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: String::new(),
            folder_type: FolderType::Physical,
            parent_id: None,
            library_id: None,
            disk_path: None,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Folder {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: name.into(),
            folder_type: FolderType::Physical,
            parent_id: None,
            library_id: None,
            disk_path: None,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
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

    pub fn with_type(mut self, folder_type: FolderType) -> Self {
        self.folder_type = folder_type;
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
        assert_eq!(folder.folder_type, FolderType::Physical);
    }

    #[test]
    fn test_folder_builder() {
        let folder = Folder::new("AI Papers")
            .with_parent("parent-123")
            .with_library("library-456")
            .with_disk_path("/papers/ai")
            .with_type(FolderType::Virtual);

        assert_eq!(folder.name, "AI Papers");
        assert_eq!(folder.parent_id, Some("parent-123".to_string()));
        assert_eq!(folder.library_id, Some("library-456".to_string()));
        assert_eq!(folder.disk_path, Some("/papers/ai".to_string()));
        assert_eq!(folder.folder_type, FolderType::Virtual);
    }
}
