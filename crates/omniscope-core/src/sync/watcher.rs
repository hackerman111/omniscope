use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

use crate::error::Result;
use crate::models::manifest::WatcherConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatcherEvent {
    /// New book file detected
    NewBookFile { path: PathBuf },
    /// Book file removed
    BookFileRemoved { path: PathBuf },
    /// Book file renamed
    BookFileRenamed { from: PathBuf, to: PathBuf },
    /// New directory created
    DirectoryCreated { path: PathBuf },
    /// Directory removed
    DirectoryRemoved { path: PathBuf },
    /// Directory renamed
    DirectoryRenamed { from: PathBuf, to: PathBuf },
}

pub struct LibraryWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    pub config: WatcherConfig,
}

impl LibraryWatcher {
    /// Start watching the library root. Connects the debouncer to a standard thread that 
    /// processes raw notify events, filters them, and emits `WatcherEvent`s through
    /// the returned MPSC channel.
    pub fn start(
        library_root: PathBuf,
        config: WatcherConfig,
    ) -> Result<(Self, mpsc::Receiver<WatcherEvent>)> {
        let (raw_tx, raw_rx) = mpsc::channel::<DebounceEventResult>();
        let (event_tx, event_rx) = mpsc::channel::<WatcherEvent>();

        // Set up the debouncer
        let mut debouncer = new_debouncer(
            Duration::from_millis(config.debounce_ms),
            move |res: DebounceEventResult| {
                let _ = raw_tx.send(res);
            },
        )?;

        // Recursive watching ignoring .libr structure
        debouncer.watcher().watch(&library_root, RecursiveMode::Recursive)?;

        // Spawn standard thread to interpret events
        let config_clone = config.clone();
        let root_clone = library_root.clone();

        std::thread::spawn(move || {
            Self::process_events(
                raw_rx,
                event_tx,
                root_clone,
                config_clone,
            );
        });

        Ok((
            Self {
                _debouncer: debouncer,
                config,
            },
            event_rx,
        ))
    }

    fn process_events(
        raw_rx: mpsc::Receiver<DebounceEventResult>,
        event_tx: mpsc::Sender<WatcherEvent>,
        _library_root: PathBuf,
        config: WatcherConfig,
    ) {
        while let Ok(res) = raw_rx.recv() {
            match res {
                Ok(events) => {
                    for event in events {
                        let path = &event.path;

                        // Ignore hidden paths (e.g. .libr)
                        if path.components().any(|c| {
                            c.as_os_str()
                                .to_str()
                                .map(|s| s.starts_with('.'))
                                .unwrap_or(false)
                        }) {
                            continue;
                        }

                        let exists = std::fs::metadata(path).is_ok();
                        let is_dir = path.is_dir();
                        let is_book = Self::is_book_extension(path, &config.watch_extensions);

                        if exists {
                            // Creation or modification
                            if is_dir {
                                let _ = event_tx.send(WatcherEvent::DirectoryCreated {
                                    path: path.clone(),
                                });
                            } else if is_book {
                                // Double check size
                                if let Ok(meta) = std::fs::metadata(path) {
                                    if meta.len() >= config.min_file_size_bytes {
                                        let _ = event_tx.send(WatcherEvent::NewBookFile {
                                            path: path.clone(),
                                        });
                                    }
                                }
                            }
                        } else {
                            if is_book {
                                let _ = event_tx.send(WatcherEvent::BookFileRemoved {
                                    path: path.clone(),
                                });
                            } else {
                                // Could be a directory deletion or non-book file
                                let _ = event_tx.send(WatcherEvent::DirectoryRemoved {
                                    path: path.clone(),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Watcher error: {:?}", e);
                }
            }
        }
    }

    fn is_book_extension(path: &Path, extensions: &[String]) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            extensions.contains(&ext_lower)
        } else {
            false
        }
    }
}
