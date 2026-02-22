pub mod bibtex;
pub mod ris;
pub mod csl;

use omniscope_core::models::book::BookCard;

pub trait Citable {
    fn to_bibtex(&self) -> String;
    fn to_ris(&self) -> String;
    fn to_csl_json(&self) -> String;
}

impl Citable for BookCard {
    fn to_bibtex(&self) -> String {
        bibtex::generate_bibtex(self)
    }

    fn to_ris(&self) -> String {
        ris::generate_ris(self)
    }

    fn to_csl_json(&self) -> String {
        csl::generate_csl_json_string(self)
    }
}
