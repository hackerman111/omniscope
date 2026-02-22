use crate::error::{Result, ScienceError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Isbn {
    pub raw: String,
    pub isbn13: String,
    pub isbn10: Option<String>,
    pub formatted: String,
}

fn strip_isbn(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}

fn check_isbn10(digits: &[u8]) -> bool {
    // digits[9] may be 10 (X)
    let sum: u32 = digits.iter().enumerate().map(|(i, &d)| (10 - i as u32) * d as u32).sum();
    sum % 11 == 0
}

fn check_isbn13(digits: &[u8]) -> bool {
    let sum: u32 = digits.iter().enumerate().map(|(i, &d)| {
        if i % 2 == 0 { d as u32 } else { d as u32 * 3 }
    }).sum();
    sum % 10 == 0
}

fn isbn10_to_isbn13(digits10: &[u8]) -> String {
    let mut d13: Vec<u8> = vec![9, 7, 8];
    d13.extend_from_slice(&digits10[..9]);
    // compute check digit
    let sum: u32 = d13.iter().enumerate().map(|(i, &d)| {
        if i % 2 == 0 { d as u32 } else { d as u32 * 3 }
    }).sum();
    let check = (10 - (sum % 10)) % 10;
    d13.push(check as u8);
    d13.iter().map(|d| d.to_string()).collect()
}

fn format_isbn13(s: &str) -> String {
    // Format as XXX-X-XXXXX-XXX-X (simplified: just add hyphens at standard positions)
    // Standard: prefix(3)-group-publisher-title-check(1)
    // For simplicity, use 978-X-XXXX-XXXX-X grouping
    if s.len() == 13 {
        format!("{}-{}-{}-{}-{}", &s[0..3], &s[3..4], &s[4..8], &s[8..12], &s[12..13])
    } else {
        s.to_string()
    }
}

impl Isbn {
    pub fn parse(input: &str) -> Result<Self> {
        let stripped = strip_isbn(input);

        if stripped.len() == 13 {
            // Parse as ISBN-13
            let digits: Vec<u8> = stripped.chars().map(|c| c as u8 - b'0').collect::<Vec<_>>();
            if digits.iter().any(|&d| d > 9) {
                return Err(ScienceError::InvalidIsbn(input.to_string()));
            }
            if !check_isbn13(&digits) {
                return Err(ScienceError::InvalidIsbn(input.to_string()));
            }
            let isbn13 = stripped.clone();
            // isbn10 only for 978 prefix
            let isbn10 = if stripped.starts_with("978") {
                let d9: Vec<u8> = digits[3..12].to_vec();
                // compute isbn10 check digit
                let sum: u32 = d9.iter().enumerate().map(|(i, &d)| (9 - i as u32) * d as u32).sum();
                let check = sum % 11;
                let check_char = if check == 10 { 'X' } else { (b'0' + check as u8) as char };
                let mut s: String = d9.iter().map(|d| d.to_string()).collect();
                s.push(check_char);
                Some(s)
            } else {
                None
            };
            let formatted = format_isbn13(&isbn13);
            return Ok(Self { raw: input.to_string(), isbn13, isbn10, formatted });
        }

        if stripped.len() == 10 {
            // Parse as ISBN-10; last char may be X
            let mut digits: Vec<u8> = Vec::with_capacity(10);
            for (i, c) in stripped.chars().enumerate() {
                if i == 9 && c == 'X' {
                    digits.push(10);
                } else if c.is_ascii_digit() {
                    digits.push(c as u8 - b'0');
                } else {
                    return Err(ScienceError::InvalidIsbn(input.to_string()));
                }
            }
            if !check_isbn10(&digits) {
                return Err(ScienceError::InvalidIsbn(input.to_string()));
            }
            let isbn10: String = stripped.clone();
            let isbn13 = isbn10_to_isbn13(&digits);
            let formatted = format_isbn13(&isbn13);
            return Ok(Self { raw: input.to_string(), isbn13, isbn10: Some(isbn10), formatted });
        }

        Err(ScienceError::InvalidIsbn(input.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_isbn13() {
        let isbn = Isbn::parse("9780306406157").unwrap();
        assert_eq!(isbn.isbn13, "9780306406157");
        assert!(isbn.isbn10.is_some());
    }

    #[test]
    fn isbn13_with_hyphens() {
        let isbn = Isbn::parse("978-0-306-40615-7").unwrap();
        assert_eq!(isbn.isbn13, "9780306406157");
    }

    #[test]
    fn valid_isbn10() {
        let isbn = Isbn::parse("0306406152").unwrap();
        assert_eq!(isbn.isbn10, Some("0306406152".to_string()));
        assert_eq!(isbn.isbn13, "9780306406157");
    }

    #[test]
    fn isbn10_with_x_check() {
        // 007462542X is a valid ISBN-10 with X check digit
        let isbn = Isbn::parse("007462542X").unwrap();
        assert_eq!(isbn.isbn10, Some("007462542X".to_string()));
    }

    #[test]
    fn invalid_check_digit() {
        assert!(Isbn::parse("9780306406158").is_err()); // wrong check digit
    }

    #[test]
    fn isbn13_979_no_isbn10() {
        // 979-1-... prefix has no ISBN-10 equivalent
        let isbn = Isbn::parse("9791032305690").unwrap();
        assert_eq!(isbn.isbn10, None);
    }
}
