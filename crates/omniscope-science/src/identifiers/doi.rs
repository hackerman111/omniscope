use crate::error::{Result, ScienceError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Doi {
    pub raw: String,
    pub normalized: String,
    pub url: String,
}

impl Doi {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        // Strip known prefixes to get the raw DOI
        let stripped = if let Some(s) = input.strip_prefix("https://doi.org/") {
            s
        } else if let Some(s) = input.strip_prefix("http://doi.org/") {
            s
        } else if let Some(s) = input.strip_prefix("https://dx.doi.org/") {
            s
        } else if let Some(s) = input.strip_prefix("http://dx.doi.org/") {
            s
        } else if let Some(s) = input.strip_prefix("doi:") {
            s.trim_start()
        } else if let Some(s) = input.strip_prefix("DOI:") {
            s.trim_start()
        } else if let Some(s) = input.strip_prefix("doi: ") {
            s
        } else if let Some(s) = input.strip_prefix("DOI: ") {
            s
        } else {
            input
        };

        // Validate: must start with "10.", contain "/", and have non-empty suffix
        if !stripped.starts_with("10.") {
            return Err(ScienceError::InvalidDoi(input.to_string()));
        }
        let slash_pos = stripped
            .find('/')
            .ok_or_else(|| ScienceError::InvalidDoi(input.to_string()))?;
        let suffix = &stripped[slash_pos + 1..];
        if suffix.is_empty() {
            return Err(ScienceError::InvalidDoi(input.to_string()));
        }

        let normalized = stripped.to_lowercase();
        let url = format!("https://doi.org/{normalized}");

        Ok(Self {
            raw: input.to_string(),
            normalized,
            url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_doi() {
        let doi = Doi::parse("10.1000/xyz123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
        assert_eq!(doi.url, "https://doi.org/10.1000/xyz123");
    }

    #[test]
    fn doi_with_https_prefix() {
        let doi = Doi::parse("https://doi.org/10.1000/xyz123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
    }

    #[test]
    fn doi_with_doi_colon_prefix() {
        let doi = Doi::parse("doi:10.1000/xyz123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
    }

    #[test]
    fn doi_with_space_after_colon() {
        let doi = Doi::parse("DOI: 10.1000/xyz123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
    }

    #[test]
    fn doi_uppercase_normalized_to_lowercase() {
        let doi = Doi::parse("10.1000/XYZ123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
    }

    #[test]
    fn reject_not_a_doi() {
        assert!(Doi::parse("not-a-doi").is_err());
    }

    #[test]
    fn reject_doi_without_suffix() {
        assert!(Doi::parse("10.1000").is_err());
    }

    #[test]
    fn reject_empty_string() {
        assert!(Doi::parse("").is_err());
    }

    #[test]
    fn doi_with_dx_doi_org() {
        let doi = Doi::parse("http://dx.doi.org/10.1000/xyz123").unwrap();
        assert_eq!(doi.normalized, "10.1000/xyz123");
    }
}
