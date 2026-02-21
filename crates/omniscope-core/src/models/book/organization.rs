use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadStatus {
    #[default]
    Unread,
    Reading,
    Read,
    Dnf,
}

impl std::fmt::Display for ReadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unread => write!(f, "unread"),
            Self::Reading => write!(f, "reading"),
            Self::Read => write!(f, "read"),
            Self::Dnf => write!(f, "dnf"),
        }
    }
}

impl std::str::FromStr for ReadStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unread" => Ok(Self::Unread),
            "reading" => Ok(Self::Reading),
            "read" => Ok(Self::Read),
            "dnf" => Ok(Self::Dnf),
            _ => Err(format!("Invalid ReadStatus: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookOrganization {
    #[serde(default)]
    pub libraries: Vec<String>,

    #[serde(default)]
    pub folders: Vec<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,

    #[serde(default)]
    pub read_status: ReadStatus,

    #[serde(default)]
    pub priority: Priority,

    #[serde(default)]
    pub custom_fields: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_status_display() {
        assert_eq!(ReadStatus::Unread.to_string(), "unread");
        assert_eq!(ReadStatus::Reading.to_string(), "reading");
        assert_eq!(ReadStatus::Read.to_string(), "read");
        assert_eq!(ReadStatus::Dnf.to_string(), "dnf");
    }

    #[test]
    fn test_read_status_from_str() {
        assert_eq!("unread".parse::<ReadStatus>().unwrap(), ReadStatus::Unread);
        assert_eq!(
            "reading".parse::<ReadStatus>().unwrap(),
            ReadStatus::Reading
        );
    }
}
