use omniscope_core::models::book::BookCard;
use omniscope_core::models::publication::DocumentType;

pub fn generate_ris(card: &BookCard) -> String {
    let mut ris = String::new();
    
    // Type
    let ty = match card.publication.as_ref().map(|p| p.doc_type).unwrap_or_default() {
        DocumentType::Article => "JOUR",
        DocumentType::Book => "BOOK",
        DocumentType::Chapter => "CHAP",
        DocumentType::ConferencePaper => "CPER",
        DocumentType::Preprint => "UNPB",
        _ => "GEN",
    };
    ris.push_str(&format!("TY  - {}\n", ty));
    
    // Title
    ris.push_str(&format!("TI  - {}\n", card.metadata.title));
    
    // Authors
    for author in &card.metadata.authors {
        ris.push_str(&format!("AU  - {}\n", author));
    }
    
    // Year
    if let Some(year) = card.metadata.year {
        ris.push_str(&format!("PY  - {}\n", year));
    }
    
    // Journal / Publication Info
    if let Some(pub_info) = &card.publication {
        if let Some(journal) = &pub_info.journal {
            ris.push_str(&format!("JO  - {}\n", journal));
        }
        if let Some(volume) = &pub_info.volume {
            ris.push_str(&format!("VL  - {}\n", volume));
        }
        if let Some(issue) = &pub_info.issue {
            ris.push_str(&format!("IS  - {}\n", issue));
        }
        if let Some(pages) = &pub_info.pages {
            // RIS often separates SP (start) and EP (end)
            if let Some((start, end)) = pages.split_once('-') {
                ris.push_str(&format!("SP  - {}\n", start.trim()));
                ris.push_str(&format!("EP  - {}\n", end.trim()));
            } else {
                ris.push_str(&format!("SP  - {}\n", pages));
            }
        }
    }
    
    if let Some(publisher) = &card.metadata.publisher {
        ris.push_str(&format!("PB  - {}\n", publisher));
    }
    
    // Identifiers
    if let Some(ids) = &card.identifiers {
        if let Some(doi) = &ids.doi {
            ris.push_str(&format!("DO  - {}\n", doi));
        }
    }
    
    for isbn in &card.metadata.isbn {
        ris.push_str(&format!("SN  - {}\n", isbn));
    }
    
    // Abstract
    if let Some(summary) = &card.ai.summary {
        ris.push_str(&format!("N2  - {}\n", summary));
    }
    
    ris.push_str("ER  - \n");
    ris
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::book::BookCard;

    #[test]
    fn test_generate_ris() {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec!["Vaswani, Ashish".to_string()];
        card.metadata.year = Some(2017);
        
        let ris = generate_ris(&card);
        assert!(ris.contains("TY  - BOOK"));
        assert!(ris.contains("TI  - Attention Is All You Need"));
        assert!(ris.contains("AU  - Vaswani, Ashish"));
        assert!(ris.contains("ER  - "));
    }
}
