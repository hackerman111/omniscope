use omniscope_core::models::book::BookCard;
use omniscope_core::models::publication::DocumentType;
use serde_json::{Value, json};

pub fn generate_csl_json(card: &BookCard) -> Value {
    let mut item = json!({
        "id": card.id.to_string(),
        "title": card.metadata.title,
    });

    // Type
    let csl_type = match card.publication.as_ref().map(|p| p.doc_type).unwrap_or_default() {
        DocumentType::Article => "article-journal",
        DocumentType::Book => "book",
        DocumentType::Chapter => "chapter",
        DocumentType::ConferencePaper => "paper-conference",
        DocumentType::Preprint => "article",
        DocumentType::Thesis => "thesis",
        DocumentType::Report => "report",
        _ => "document",
    };
    item["type"] = json!(csl_type);

    // Authors
    let authors: Vec<Value> = card.metadata.authors.iter().map(|s| {
        if let Some((family, given)) = s.split_once(',') {
            json!({
                "family": family.trim(),
                "given": given.trim()
            })
        } else if let Some((given, family)) = s.rsplit_once(' ') {
            json!({
                "family": family.trim(),
                "given": given.trim()
            })
        } else {
            json!({ "literal": s })
        }
    }).collect();
    
    if !authors.is_empty() {
        item["author"] = json!(authors);
    }

    // Issued
    if let Some(year) = card.metadata.year {
        item["issued"] = json!({
            "date-parts": [[year]]
        });
    }

    // Container / Publication Info
    if let Some(pub_info) = &card.publication {
        if let Some(journal) = &pub_info.journal {
            item["container-title"] = json!(journal);
        }
        if let Some(volume) = &pub_info.volume {
            item["volume"] = json!(volume);
        }
        if let Some(issue) = &pub_info.issue {
            item["issue"] = json!(issue);
        }
        if let Some(pages) = &pub_info.pages {
            item["page"] = json!(pages);
        }
    }

    if let Some(publisher) = &card.metadata.publisher {
        item["publisher"] = json!(publisher);
    }

    // Identifiers
    if let Some(ids) = &card.identifiers
        && let Some(doi) = &ids.doi {
            item["DOI"] = json!(doi);
        }

    if !card.metadata.isbn.is_empty() {
        item["ISBN"] = json!(card.metadata.isbn.join(", "));
    }

    // Abstract
    if let Some(summary) = &card.ai.summary {
        item["abstract"] = json!(summary);
    }

    item
}

pub fn generate_csl_json_string(card: &BookCard) -> String {
    serde_json::to_string_pretty(&generate_csl_json(card)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::book::BookCard;

    #[test]
    fn test_generate_csl_json() {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec!["Vaswani, Ashish".to_string()];
        card.metadata.year = Some(2017);
        
        let csl = generate_csl_json(&card);
        assert_eq!(csl["title"], "Attention Is All You Need");
        assert_eq!(csl["author"][0]["family"], "Vaswani");
        assert_eq!(csl["issued"]["date-parts"][0][0], 2017);
    }
}
