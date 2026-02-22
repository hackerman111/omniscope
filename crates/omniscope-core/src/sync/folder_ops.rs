use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{Folder, BookCard};
use crate::storage::database::Database;
use crate::storage::library_root::LibraryRoot;
use rusqlite::Connection;

pub struct FolderOps<'a> {
    library: &'a LibraryRoot,
    db: &'a Database,
}

impl<'a> FolderOps<'a> {
    pub fn new(library: &'a LibraryRoot, db: &'a Database) -> Self {
        Self { library, db }
    }

    /// Rename a folder on disk and recursively update all descendant paths in DB
    pub fn rename_folder(&self, folder_id: &str, new_name: &str) -> Result<()> {
        let folder = self.db.find_folder_by_id(folder_id)?.ok_or_else(|| {
            crate::error::OmniscopeError::DirectoryNotFound(format!("Folder not found: {}", folder_id))
        })?;

        let old_disk_path = folder.disk_path.as_deref().unwrap_or("").to_string();
        if old_disk_path.is_empty() {
            // Nothing to do on disk
            return Ok(());
        }

        let old_path = self.library.root().join(&old_disk_path);
        let new_rel_path = if let Some(parent) = Path::new(&old_disk_path).parent() {
            if parent.as_os_str().is_empty() {
                PathBuf::from(new_name)
            } else {
                parent.join(new_name)
            }
        } else {
            PathBuf::from(new_name)
        };

        let new_path = self.library.root().join(&new_rel_path);

        if old_path.exists() && !new_path.exists() {
            std::fs::rename(&old_path, &new_path)?;
        }

        // Apply bulk SQLite recursive update
        self.db.update_folder_path_recursive(folder_id, &old_disk_path, new_rel_path.to_string_lossy().as_ref())?;

        // Now find all books whose files were inside old_disk_path and update their paths
        let affected_books = self.db.find_books_by_path_prefix(&old_disk_path)?;
        for mut book in affected_books {
            let mut changed = false;
            if let Some(mut file_info) = book.file.clone() {
                if file_info.path.starts_with(&old_disk_path) {
                    let remain = &file_info.path[old_disk_path.len()..];
                    let remain = remain.trim_start_matches('/');
                    file_info.path = if remain.is_empty() {
                        new_rel_path.to_string_lossy().to_string()
                    } else {
                        new_rel_path.join(remain).to_string_lossy().to_string()
                    };
                    book.file = Some(file_info);
                    changed = true;
                }
            }

            if let crate::models::FilePresence::Present { path: old_pres_path, hash, size_bytes } = &book.file_presence {
                let p_str = old_pres_path.to_string_lossy().to_string();
                if p_str.starts_with(&old_disk_path) {
                    let remain = &p_str[old_disk_path.len()..];
                    let remain = remain.trim_start_matches('/');
                    let new_p = if remain.is_empty() {
                        new_rel_path.clone()
                    } else {
                        new_rel_path.join(remain)
                    };
                    book.file_presence = crate::models::FilePresence::Present {
                        path: new_p,
                        size_bytes: *size_bytes,
                        hash: hash.clone(),
                    };
                    changed = true;
                }
            }

            if changed {
                self.db.upsert_book(&book)?;
                let _ = crate::storage::json_cards::save_card(&self.library.cards_dir(), &book);
            }
        }

        // Also update the name field
        let mut updated_folder = folder;
        updated_folder.name = new_name.to_string();
        self.db.update_folder(&updated_folder)?;

        Ok(())
    }

    /// Move a folder to a new parent on disk and update paths in DB
    pub fn move_folder(&self, folder_id: &str, new_parent_id: Option<&str>) -> Result<()> {
        let folder = self.db.find_folder_by_id(folder_id)?.ok_or_else(|| {
            crate::error::OmniscopeError::DirectoryNotFound(format!("Folder {} not found", folder_id))
        })?;

        let old_disk_path = folder.disk_path.as_deref().unwrap_or("").to_string();
        
        let new_parent_path = if let Some(pid) = new_parent_id {
            let p_folder = self.db.find_folder_by_id(pid)?.ok_or_else(|| {
                crate::error::OmniscopeError::DirectoryNotFound(format!("Parent {} not found", pid))
            })?;
            p_folder.disk_path.unwrap_or_default()
        } else {
            "".to_string()
        };

        let new_rel_path = if new_parent_path.is_empty() {
            PathBuf::from(&folder.name)
        } else {
            PathBuf::from(&new_parent_path).join(&folder.name)
        };

        let old_path = self.library.root().join(&old_disk_path);
        let new_path = self.library.root().join(&new_rel_path);

        if old_path.exists() && !new_path.exists() {
            if let Some(parent_dir) = new_path.parent() {
                std::fs::create_dir_all(parent_dir)?;
            }
            std::fs::rename(&old_path, &new_path)?;
        }

        self.db.update_folder_path_recursive(folder_id, &old_disk_path, new_rel_path.to_string_lossy().as_ref())?;

        // Find and update all books recursively inside the moved folder
        let affected_books = self.db.find_books_by_path_prefix(&old_disk_path)?;
        for mut book in affected_books {
            let mut changed = false;
            if let Some(mut file_info) = book.file.clone() {
                if file_info.path.starts_with(&old_disk_path) {
                    let remain = &file_info.path[old_disk_path.len()..];
                    let remain = remain.trim_start_matches('/');
                    file_info.path = if remain.is_empty() {
                        new_rel_path.to_string_lossy().to_string()
                    } else {
                        new_rel_path.join(remain).to_string_lossy().to_string()
                    };
                    book.file = Some(file_info);
                    changed = true;
                }
            }

            if let crate::models::FilePresence::Present { path: old_pres_path, hash, size_bytes } = &book.file_presence {
                let p_str = old_pres_path.to_string_lossy().to_string();
                if p_str.starts_with(&old_disk_path) {
                    let remain = &p_str[old_disk_path.len()..];
                    let remain = remain.trim_start_matches('/');
                    let new_p = if remain.is_empty() {
                        new_rel_path.clone()
                    } else {
                        new_rel_path.join(remain)
                    };
                    book.file_presence = crate::models::FilePresence::Present {
                        path: new_p,
                        size_bytes: *size_bytes,
                        hash: hash.clone(),
                    };
                    changed = true;
                }
            }

            if changed {
                self.db.upsert_book(&book)?;
                let _ = crate::storage::json_cards::save_card(&self.library.cards_dir(), &book);
            }
        }

        let mut updated_folder = folder;
        updated_folder.parent_id = new_parent_id.map(|s| s.to_string());
        self.db.update_folder(&updated_folder)?;

        Ok(())
    }

    /// Create a new folder
    pub fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<crate::models::Folder> {
        // Validate parent exists
        let parent_disk_path = if let Some(pid) = parent_id {
            if let Some(parent) = self.db.find_folder_by_id(pid)? {
                parent.disk_path
            } else {
                return Err(crate::error::OmniscopeError::DirectoryNotFound(format!("Parent {} not found", pid)));
            }
        } else {
            None
        };

        let new_rel_path = if let Some(ref pp) = parent_disk_path {
            Path::new(pp).join(name)
        } else {
            PathBuf::from(name)
        };

        let disk_path_str = new_rel_path.to_string_lossy().to_string();
        let full_path = self.library.root().join(&new_rel_path);

        // Create directory on disk
        if !full_path.exists() {
            std::fs::create_dir_all(&full_path)?;
        }

        // Insert into DB
        let id_string = self.db.create_folder(name, parent_id, None)?;
        
        let folder = self.db.find_folder_by_id(&id_string)?
            .unwrap_or_else(|| {
                let mut f = crate::models::Folder::new(name);
                f.id = id_string.clone();
                f.parent_id = parent_id.map(|s| s.to_string());
                f.disk_path = Some(disk_path_str.clone());
                f
            });

        // Try to back-patch disk_path if it wasn't populated by default creation
        if folder.disk_path.is_none() {
            let mut patched = folder.clone();
            patched.disk_path = Some(disk_path_str);
            self.db.update_folder(&patched)?;
            return Ok(patched);
        }

        Ok(folder)
    }

    /// Delete a folder from disk and cascade delete in DB
    pub fn delete_folder(&self, folder_id: &str, keep_files: bool) -> Result<()> {
        if let Some(folder) = self.db.find_folder_by_id(folder_id)? {
            if !keep_files {
                if let Some(disk_path) = folder.disk_path {
                    let path = self.library.root().join(disk_path);
                    if path.exists() {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
        
        self.db.delete_folder(folder_id)?;
        Ok(())
    }

    /// Move a book to a new folder on disk and update its location in DB
    pub fn move_book(&self, book_id: &str, new_folder_id: Option<&str>) -> Result<()> {
        let mut book = self.db.get_book_card(book_id)?;
        
        let new_folder_path = if let Some(fid) = new_folder_id {
            let folder = self.db.find_folder_by_id(fid)?.ok_or_else(|| {
                crate::error::OmniscopeError::DirectoryNotFound(format!("Folder {} not found", fid))
            })?;
            folder.disk_path
        } else {
            None
        };

        if let Some(ref file_info) = book.file {
            let old_path = self.library.root().join(&file_info.path);
            let filename = Path::new(&file_info.path).file_name().unwrap_or_default();
            
            let new_rel_path = if let Some(ref fp) = new_folder_path {
                PathBuf::from(fp).join(filename)
            } else {
                PathBuf::from(filename)
            };
            
            let new_path = self.library.root().join(&new_rel_path);

            if old_path.exists() && !new_path.exists() {
                if let Some(parent_dir) = new_path.parent() {
                    std::fs::create_dir_all(parent_dir)?;
                }
                std::fs::rename(&old_path, &new_path)?;
            }

            // Update file paths in BookCard
            let mut updated_file_info = file_info.clone();
            updated_file_info.path = new_rel_path.to_string_lossy().to_string();
            book.file = Some(updated_file_info);
            
            // Also update presence
            if let crate::models::FilePresence::Present { hash, size_bytes, .. } = &book.file_presence {
                book.file_presence = crate::models::FilePresence::Present {
                    path: new_rel_path,
                    size_bytes: *size_bytes,
                    hash: hash.clone(),
                };
            }
        }

        book.folder_id = new_folder_id.map(|s| s.to_string());
        self.db.upsert_book(&book)?;
        let _ = crate::storage::json_cards::save_card(&self.library.cards_dir(), &book);

        Ok(())
    }
}
