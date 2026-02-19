//! Omniscope FFI — bindings to libmagic, poppler, libepub.
//! Stub for Phase 1+ implementation.

pub mod magic {
    //! libmagic MIME type detection. Full implementation in Phase 1.

    /// Detect MIME type from file bytes (stub — returns None until libmagic is wired).
    pub fn detect_mime(_path: &std::path::Path) -> Option<String> {
        None
    }
}

pub mod poppler {
    //! PDF metadata extraction via poppler. Full implementation in Phase 3.
}

pub mod epub {
    //! EPUB metadata extraction via libepub. Full implementation in Phase 3.
}
