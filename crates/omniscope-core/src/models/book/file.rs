use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookFile {
    pub path: String,
    pub format: FileFormat,
    pub size_bytes: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_sha256: Option<String>,

    pub added_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Pdf,
    Epub,
    Djvu,
    Mobi,
    Fb2,
    Txt,
    Html,
    Azw3,
    Cbz,
    Cbr,
    Other,
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Djvu => "djvu",
            Self::Mobi => "mobi",
            Self::Fb2 => "fb2",
            Self::Txt => "txt",
            Self::Html => "html",
            Self::Azw3 => "azw3",
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Other => "other",
        };
        write!(f, "{s}")
    }
}

impl FileFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "djvu" => Self::Djvu,
            "mobi" => Self::Mobi,
            "fb2" => Self::Fb2,
            "txt" | "text" => Self::Txt,
            "html" | "htm" => Self::Html,
            "azw3" => Self::Azw3,
            "cbz" => Self::Cbz,
            "cbr" => Self::Cbr,
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_format_from_extension() {
        assert_eq!(FileFormat::from_extension("pdf"), FileFormat::Pdf);
        assert_eq!(FileFormat::from_extension("PDF"), FileFormat::Pdf);
        assert_eq!(FileFormat::from_extension("epub"), FileFormat::Epub);
        assert_eq!(FileFormat::from_extension("xyz"), FileFormat::Other);
    }
}
