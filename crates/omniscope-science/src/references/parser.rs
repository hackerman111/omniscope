use once_cell::sync::Lazy;
use regex::Regex;

static REFERENCES_HEADING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?im)^\s*(?:\d{1,2}(?:[.)]\s*)?)?(?:references|bibliography|works\s+cited|литература|список\s+литературы)\s*:?\s*$",
    )
    .expect("valid references heading regex")
});

static SECTION_END_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?im)^\s*(?:\d{1,2}(?:[.)]\s*)?)?(?:appendix(?:\s+[a-z0-9]+)?|supplementary(?:\s+materials?)?|acknowledg(?:e)?ments?|приложени[ея]|благодарности)\s*:?\s*$",
    )
    .expect("valid references section end regex")
});

static NUMBERED_REFERENCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?:\[\d+\]|\(\d+\)|\d+[.)\]])\s*").expect("valid numbered reference regex")
});

static EMPTY_LINE_SPLIT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n\s*\n+").expect("valid empty-line split regex"));
static NUMBERED_REFERENCE_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:\[\d+\]|\(\d+\)|\d+[.)\]])\s+")
        .expect("valid numbered reference block regex")
});
static REFERENCE_YEAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").expect("valid reference year regex"));

pub fn find_references_section(text: &str) -> Option<&str> {
    let Some(heading_match) = REFERENCES_HEADING_RE.find(text) else {
        return fallback_references_section_without_heading(text);
    };

    let mut section_start = heading_match.end();
    while let Some(ch) = text[section_start..].chars().next() {
        if ch == '\n' || ch == '\r' {
            section_start += ch.len_utf8();
            continue;
        }
        break;
    }

    let tail = &text[section_start..];
    let section_end = SECTION_END_RE
        .find(tail)
        .map(|end_match| end_match.start())
        .unwrap_or(tail.len());
    let section = &tail[..section_end];

    if section.trim().is_empty() {
        None
    } else {
        Some(section)
    }
}

fn fallback_references_section_without_heading(text: &str) -> Option<&str> {
    if text.trim().is_empty() {
        return None;
    }

    let matches = NUMBERED_REFERENCE_BLOCK_RE
        .find_iter(text)
        .collect::<Vec<_>>();
    if matches.len() < 2 {
        return None;
    }

    let start_idx = matches.len().saturating_sub(12);

    let section = &text[matches[start_idx].start()..];
    if section.trim().is_empty() {
        None
    } else {
        Some(section)
    }
}

pub fn parse_reference_lines(section: &str) -> Vec<String> {
    let numbered = parse_numbered_references(section);
    if !numbered.is_empty() {
        return numbered;
    }
    parse_unnumbered_references(section)
}

fn parse_numbered_references(section: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut saw_numbering = false;

    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                push_reference(&mut entries, &mut current);
            }
            continue;
        }

        if let Some(prefix) = NUMBERED_REFERENCE_RE.find(trimmed)
            && prefix.start() == 0
        {
            saw_numbering = true;
            if !current.is_empty() {
                push_reference(&mut entries, &mut current);
            }

            let body = trimmed[prefix.end()..].trim();
            current.push_str(body);
            continue;
        }

        if saw_numbering {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
        }
    }

    if !current.is_empty() {
        push_reference(&mut entries, &mut current);
    }

    if saw_numbering { entries } else { Vec::new() }
}

fn parse_unnumbered_references(section: &str) -> Vec<String> {
    let entries = EMPTY_LINE_SPLIT_RE
        .split(section)
        .map(normalize_whitespace)
        .filter(|entry| entry.chars().count() > 20)
        .collect::<Vec<_>>();
    if entries.len() >= 2 {
        return entries;
    }

    let mut parsed = Vec::new();
    let mut current = String::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                push_reference(&mut parsed, &mut current);
            }
            continue;
        }

        if !current.is_empty() && looks_like_unnumbered_reference_start(trimmed) {
            push_reference(&mut parsed, &mut current);
        } else if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
    }

    if !current.is_empty() {
        push_reference(&mut parsed, &mut current);
    }

    if parsed.len() >= entries.len() {
        parsed
    } else {
        entries
    }
}

fn looks_like_unnumbered_reference_start(line: &str) -> bool {
    let has_year = REFERENCE_YEAR_RE.is_match(line);
    let starts_with_letter = line
        .chars()
        .next()
        .map(|ch| ch.is_ascii_alphabetic())
        .unwrap_or(false);
    let has_author_separator = line.contains(',') || line.contains(" and ") || line.contains('&');
    starts_with_letter && has_year && has_author_separator
}

fn push_reference(entries: &mut Vec<String>, current: &mut String) {
    let normalized = normalize_whitespace(current);
    current.clear();
    if normalized.chars().count() > 20 {
        entries.push(normalized);
    }
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_parses_numbered_references() {
        let text = r#"
Introduction
References
[1] Vaswani, A. et al. Attention Is All You Need. NeurIPS (2017).
[2] Devlin, J. et al. BERT: Pre-training of Deep Bidirectional Transformers.
"#;

        let section = find_references_section(text).expect("references section should exist");
        let refs = parse_reference_lines(section);

        assert_eq!(refs.len(), 2);
        assert!(refs[0].contains("Attention Is All You Need"));
        assert!(refs[1].contains("BERT"));
    }

    #[test]
    fn parses_unnumbered_references_split_by_empty_lines() {
        let text = r#"
Bibliography
Vaswani, A. et al. Attention Is All You Need. NeurIPS, 2017.

Devlin, J. et al. BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding.
"#;

        let section = find_references_section(text).expect("references section should exist");
        let refs = parse_reference_lines(section);

        assert_eq!(refs.len(), 2);
        assert!(refs[0].starts_with("Vaswani"));
        assert!(refs[1].contains("Language Understanding"));
    }

    #[test]
    fn stops_references_before_appendix_heading() {
        let text = r#"
Main text
References
1. Vaswani, A. et al. Attention Is All You Need. NeurIPS, 2017.
2. Brown, T. et al. Language Models are Few-Shot Learners. NeurIPS, 2020.

Appendix
Extra material that must not be parsed as references.
"#;

        let section = find_references_section(text).expect("references section should exist");
        assert!(!section.contains("Extra material"));

        let refs = parse_reference_lines(section);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn falls_back_to_tail_numbered_block_without_heading() {
        let text = r#"
Main body text.
Some conclusion.

[1] Goodfellow, I. et al. Deep Learning. MIT Press, 2016.
[2] LeCun, Y., Bengio, Y., Hinton, G. Deep learning. Nature, 2015.
[3] Srivastava, N. et al. Dropout: A Simple Way to Prevent Neural Networks from Overfitting.
"#;

        let section = find_references_section(text).expect("fallback section should be found");
        let refs = parse_reference_lines(section);
        assert_eq!(refs.len(), 3);
        assert!(refs[0].contains("Deep Learning"));
    }

    #[test]
    fn parses_compact_unnumbered_references_without_blank_lines() {
        let section = r#"
Goodfellow, I., Bengio, Y., and Courville, A. (2016). Deep Learning. MIT Press.
LeCun, Y., Bengio, Y., and Hinton, G. (2015). Deep learning. Nature.
Srivastava, N., Hinton, G., et al. (2014). Dropout: A Simple Way to Prevent Neural Networks from Overfitting.
"#;

        let refs = parse_reference_lines(section);
        assert_eq!(refs.len(), 3);
        assert!(refs[1].contains("Nature"));
    }
}
