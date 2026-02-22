use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};

use omniscope_core::{
    AppConfig, BookCard, Database, GlobalConfig, LibraryRoot,
    init_library, InitOptions,
    scan_library, ScanOptions,
    FolderTemplate, scaffold_template, sync_folders,
};
use omniscope_science::{
    ScienceConfig, enrichment::pipeline::EnrichmentPipeline,
    sources::ExternalSource, sources::crossref::CrossRefSource,
};
use omniscope_tui::app::App;
