use std::path::Path;
use crate::error::Result;
use crate::identifiers::{doi::Doi, arxiv::ArxivId, isbn::Isbn};

pub fn extract_dois_from_text(_text: &str) -> Vec<Doi> {
    todo!()
}

pub fn extract_arxiv_ids_from_text(_text: &str) -> Vec<ArxivId> {
    todo!()
}

pub fn extract_isbn_from_text(_text: &str) -> Option<Isbn> {
    todo!()
}

pub fn find_doi_in_first_page(_pdf_path: &Path) -> Result<Doi> {
    todo!()
}

pub fn find_arxiv_id_in_pdf(_pdf_path: &Path) -> Result<ArxivId> {
    todo!()
}
