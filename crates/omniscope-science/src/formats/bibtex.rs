use omniscope_core::models::book::BookCard;
use omniscope_core::models::publication::DocumentType;

pub fn generate_bibtex(card: &BookCard) -> String {
    let cite_key = generate_cite_key(card);
    let entry_type = map_doc_type(&card.publication.as_ref().map(|p| p.doc_type).unwrap_or_default());
    
    let mut bib = format!("@{}{{{},\n", entry_type, cite_key);
    
    // Title
    bib.push_str(&format!("  title = {{{}}},\n", escape_bibtex(&card.metadata.title)));
    
    // Authors
    if !card.metadata.authors.is_empty() {
        let authors = card.metadata.authors.join(" and ");
        bib.push_str(&format!("  author = {{{}}},\n", escape_bibtex(&authors)));
    }
    
    // Year
    if let Some(year) = card.metadata.year {
        bib.push_str(&format!("  year = {{{}}},\n", year));
    }
    
    // Journal / Publisher / Booktitle
    if let Some(pub_info) = &card.publication {
        if let Some(journal) = &pub_info.journal {
            bib.push_str(&format!("  journal = {{{}}},\n", escape_bibtex(journal)));
        }
        if let Some(volume) = &pub_info.volume {
            bib.push_str(&format!("  volume = {{{}}},\n", escape_bibtex(volume)));
        }
        if let Some(issue) = &pub_info.issue {
            bib.push_str(&format!("  number = {{{}}},\n", escape_bibtex(issue)));
        }
        if let Some(pages) = &pub_info.pages {
            bib.push_str(&format!("  pages = {{{}}},\n", escape_bibtex(pages)));
        }
    }
    
    if let Some(publisher) = &card.metadata.publisher {
        bib.push_str(&format!("  publisher = {{{}}},\n", escape_bibtex(publisher)));
    }
    
    // Identifiers
    if let Some(ids) = &card.identifiers {
        if let Some(doi) = &ids.doi {
            bib.push_str(&format!("  doi = {{{}}},\n", escape_bibtex(doi)));
        }
        if let Some(arxiv) = &ids.arxiv_id {
            bib.push_str(&format!("  eprint = {{{}}},\n", escape_bibtex(arxiv)));
            bib.push_str("  archivePrefix = {arXiv},\n");
        }
    }
    
    // Abstract (optional note)
    if let Some(summary) = &card.ai.summary {
        bib.push_str(&format!("  abstract = {{{}}},\n", escape_bibtex(summary)));
    }

    bib.push_str("}\n");
    bib
}

fn generate_cite_key(card: &BookCard) -> String {
    let author = card.metadata.authors.first()
        .map(|s| s.split(',').next().unwrap_or(s).split(' ').next().unwrap_or(s).to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    
    let year = card.metadata.year.map(|y| y.to_string()).unwrap_or_default();
    
    let title_word = card.metadata.title.split_whitespace().next()
        .map(|s| s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .unwrap_or_default();
    
    format!("{}{}{}", author, year, title_word)
}

fn map_doc_type(doc_type: &DocumentType) -> &'static str {
    match doc_type {
        DocumentType::Article => "article",
        DocumentType::Book => "book",
        DocumentType::Chapter => "inbook",
        DocumentType::ConferencePaper => "inproceedings",
        DocumentType::Preprint => "unpublished",
        DocumentType::Thesis => "phdthesis",
        DocumentType::Report => "techreport",
        _ => "misc",
    }
}

fn escape_bibtex(s: &str) -> String {
    s.replace('&', "\\&")
     .replace('_', "\\_")
     .replace('$', "\\$")
     .replace('%', "\\%")
     .replace('#', "\\#")
     .replace('{', "\\{")
     .replace('}', "\\}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::book::BookCard;

    #[test]
    fn test_generate_bibtex() {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec!["Vaswani, Ashish".to_string()];
        card.metadata.year = Some(2017);
        
        let bib = generate_bibtex(&card);
        assert!(bib.contains("@book{vaswani2017attention"));
        assert!(bib.contains("title = {Attention Is All You Need}"));
        assert!(bib.contains("author = {Vaswani, Ashish}"));
    }
}
