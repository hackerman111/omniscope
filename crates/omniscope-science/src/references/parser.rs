use regex::Regex;
use once_cell::sync::Lazy;

static REF_HEADER_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^(\d+\.?\s*)?(References|Bibliography|Works Cited|Литература|Список литературы)\s*$").unwrap()
});

static SECTION_END_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^(\d+\.?\s*)?(Appendix|Supplementary|Acknowledgements|Приложение)\s*$").unwrap()
});

pub fn find_references_section(text: &str) -> Option<&str> {
    if let Some(m) = REF_HEADER_REGEX.find(text) {
        let start = m.end();
        let suffix = &text[start..];
        
        // Find end of section
        if let Some(end_m) = SECTION_END_REGEX.find(suffix) {
            return Some(&suffix[..end_m.start()]);
        }
        
        return Some(suffix);
    }
    None
}

pub fn parse_reference_lines(section: &str) -> Vec<String> {
    let mut refs = Vec::new();
    
    // Strategy 1: Numbered list [1], 1., 1)
    let numbered_regex = Regex::new(r"(?m)^(\[[0-9]+\]|\d+\.|\d+\))\s+").unwrap();
    
    // Check if it looks numbered
    let matches: Vec<_> = numbered_regex.find_iter(section).collect();
    if matches.len() >= 2 {
        let mut last_end = 0;
        for m in matches {
            if last_end > 0 {
                let s = &section[last_end..m.start()];
                let cleaned = s.replace('\n', " ").trim().to_string();
                if cleaned.len() > 20 {
                    refs.push(cleaned);
                }
            }
            last_end = m.end(); // Start of content
        }
        // Last one
        if last_end > 0 {
            let s = &section[last_end..];
            let cleaned = s.replace('\n', " ").trim().to_string();
            if cleaned.len() > 20 {
                refs.push(cleaned);
            }
        }
        return refs;
    }

    // Strategy 2: Empty lines
    for block in section.split("\n\n") {
        let cleaned = block.replace('\n', " ").trim().to_string();
        if cleaned.len() > 20 {
            refs.push(cleaned);
        }
    }
    
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_references_section() {
        let text = "Content...\nReferences\n[1] Ref 1\n[2] Ref 2\nAppendix\nMore...";
        let section = find_references_section(text).unwrap();
        assert!(section.contains("[1] Ref 1"));
        assert!(!section.contains("Appendix"));
    }

    #[test]
    fn test_parse_numbered_references() {
        let text = "[1] Author A. Title of the first paper.\n[2] Author B.\nTitle B continued.";
        let refs = parse_reference_lines(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], "Author A. Title of the first paper.");
        assert_eq!(refs[1], "Author B. Title B continued.");
    }
}
