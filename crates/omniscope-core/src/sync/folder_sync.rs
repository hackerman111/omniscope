use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::storage::database::Database;
use crate::storage::library_root::LibraryRoot;
use crate::file_import;

#[derive(Debug, Clone, PartialEq)]
pub enum SyncResolution {
    DiskWins,
    DatabaseWins,
    Interactive,
}

#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    pub new_on_disk: Vec<String>,
    pub missing_on_disk: Vec<String>,
    pub in_sync: usize,
    pub untracked_files: Vec<PathBuf>,
}

impl SyncReport {
    pub fn is_clean(&self) -> bool {
        self.new_on_disk.is_empty()
            && self.missing_on_disk.is_empty()
            && self.untracked_files.is_empty()
    }
}

pub struct FolderSync<'a> {
    library: &'a LibraryRoot,
    db: &'a Database,
}

impl<'a> FolderSync<'a> {
    pub fn new(library: &'a LibraryRoot, db: &'a Database) -> Self {
        Self { library, db }
    }

    pub fn full_scan(&self) -> Result<SyncReport> {
        let root = self.library.root();

        // 1. Discover directories on disk
        let disk_folders = self.scan_disk_directories(root)?;

        // 2. Get folders from DB
        let db_folders = self.db.list_all_folder_paths()?;
        let db_set: HashSet<String> = db_folders.into_iter().collect();

        let mut report = SyncReport::default();

        for folder in &disk_folders {
            if db_set.contains(folder) {
                report.in_sync += 1;
            } else {
                report.new_on_disk.push(folder.clone());
            }
        }

        let disk_set: HashSet<&str> = disk_folders.iter().map(|s| s.as_str()).collect();
        for db_folder in db_set.iter() {
            if !disk_set.contains(db_folder.as_str()) {
                report.missing_on_disk.push(db_folder.clone());
            }
        }

        // 3. Find untracked files
        report.untracked_files = self.scan_untracked_files()?;

        Ok(report)
    }

    pub fn apply_sync(&self, report: &SyncReport, strategy: SyncResolution) -> Result<()> {
        match strategy {
            SyncResolution::DiskWins => {
                // If Disk wins, we add what's new on disk to DB
                for new_dir in &report.new_on_disk {
                    let folder_name = Path::new(new_dir)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(new_dir);
                        
                    let parent_rel = Path::new(new_dir)
                        .parent()
                        .and_then(|p| p.to_str())
                        .filter(|s| !s.is_empty());
                        
                    let parent_id = if let Some(parent) = parent_rel {
                        self.db.find_folder_by_disk_path(parent).ok().flatten()
                    } else {
                        None
                    };

                    self.db.create_folder_with_path(folder_name, parent_id.as_deref(), None, new_dir)?;
                }

                // If Disk wins, we remove from DB what's missing on disk
                for missing_dir in &report.missing_on_disk {
                    if let Ok(Some(id)) = self.db.find_folder_by_disk_path(missing_dir) {
                        self.db.delete_folder(&id)?;
                    }
                }
            }
            SyncResolution::DatabaseWins => {
                // If Database wins, we create directories on disk that are missing
                for missing_dir in &report.missing_on_disk {
                    let disk_path = self.library.root().join(missing_dir);
                    std::fs::create_dir_all(&disk_path)?;
                }

                // If Database wins, we delete directories from disk that are only on disk (not in DB)
                // Warning: destructive!
                for new_dir in &report.new_on_disk {
                    let disk_path = self.library.root().join(new_dir);
                    if disk_path.exists() {
                        let _ = std::fs::remove_dir_all(&disk_path);
                    }
                }
            }
            SyncResolution::Interactive => {
                // Do nothing automatically in library core, expecting caller to resolve
            }
        }
        Ok(())
    }

    fn scan_disk_directories(&self, root: &Path) -> Result<Vec<String>> {
        let mut result = Vec::new();
        self.scan_dirs_recursive(root, root, &mut result)?;
        Ok(result)
    }

    fn scan_dirs_recursive(&self, root: &Path, current: &Path, result: &mut Vec<String>) -> Result<()> {
        let entries = match std::fs::read_dir(current) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with('.') {
                continue;
            }

            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                let rel_str = rel_str.replace('\\', "/");
                result.push(rel_str);
            }

            self.scan_dirs_recursive(root, &path, result)?;
        }

        Ok(())
    }

    fn scan_untracked_files(&self) -> Result<Vec<PathBuf>> {
        let root = self.library.root();
        let tracked = self.db.list_all_file_paths()?;
        let tracked_set: HashSet<String> = tracked.into_iter().collect();
        let disk_files = file_import::scan_directory(root, true)?;

        let mut untracked = Vec::new();
        for card in &disk_files {
            if let Some(ref file) = card.file {
                if !tracked_set.contains(&file.path) {
                    untracked.push(PathBuf::from(&file.path));
                }
            }
        }
        Ok(untracked)
    }
}
