use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    #[default]
    Book,
    Article,
    ConferencePaper,
    Preprint,
    Thesis,
    Report,
    Dataset,
    Software,
    Patent,
    Standard,
    Chapter,
    MagazineArticle,
    WebPage,
    Other,
}

impl std::fmt::Display for DocumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DocumentType::*;
        let s = match self {
            Book => "book",
            Article => "article",
            ConferencePaper => "conference_paper",
            Preprint => "preprint",
            Thesis => "thesis",
            Report => "report",
            Dataset => "dataset",
            Software => "software",
            Patent => "patent",
            Standard => "standard",
            Chapter => "chapter",
            MagazineArticle => "magazine_article",
            WebPage => "web_page",
            Other => "other",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookPublication {
    #[serde(default)]
    pub doc_type: DocumentType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conference: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_display() {
        assert_eq!(DocumentType::Book.to_string(), "book");
        assert_eq!(DocumentType::Article.to_string(), "article");
        assert_eq!(
            DocumentType::ConferencePaper.to_string(),
            "conference_paper"
        );
    }
}
