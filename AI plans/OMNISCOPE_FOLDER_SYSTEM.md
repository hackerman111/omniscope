# 🗂️ OMNISCOPE — Система папок: Полная спецификация

> **Этот документ** описывает архитектуру и реализацию системы папок Omniscope:
> двустороннюю синхронизацию с диском, vim-управление, автосканирование,
> режим просмотра папок в центральной панели и поддержку виртуальных книг.
>
> **Читать вместе с:** `OMNISCOPE_STORAGE.md`, `OMNISCOPE_UI_DESIGN_PLAN.md`,
> `Omniscope_VIM_MOTIONS.md §1-6`, `OMNISCOPE_MASTER_PLAN.md §1.5`

---

## 0. Философия и инварианты

```
"Папка = директория на диске"     Каждая физическая папка = реальная директория.
"Перемести в TUI — переместишь файл"  Все операции над папками — двусторонние.
"Виртуальное ≠ второсортное"      Ghost-книги (без файла) — полноправные участники.
"Диск — главный источник правды"  При конфликте: состояние диска побеждает.
"Никаких сюрпризов"               Все деструктивные операции — с preview + undo.
```

### Таксономия объектов

Прежде всего — понять что с чем работаем. В системе существуют **три типа папок**
и **три типа книг**. Всё остальное — следствие этой типизации.

```
ТИПЫ ПАПОК
─────────────────────────────────────────────────────────────────────────
PhysicalFolder    Реальная директория на диске. Путь существует в ФС.
                  Создание → mkdir. Переименование → mv. Удаление → rm -r.
                  Примеры: ~/Books/programming/rust/, ~/Books/ml-papers/

VirtualFolder     Метаданные-только. Нет директории на диске.
                  Аналог "плейлиста" — способ группировки без переноса файлов.
                  Хранится только в SQLite. Иконка ⊕.
                  Примеры: "Reading List", "Thesis 2025", "Favorites"

LibraryRoot       Корень библиотеки (.libr/). Особый тип — PhysicalFolder
                  с манифестом. Всегда верхний уровень иерархии.

ТИПЫ КНИГ
─────────────────────────────────────────────────────────────────────────
PhysicalBook      BookCard + файл существует на диске. Полноценная книга.
                  Иконка: 󰈙 (pdf) / 󰃴 (epub) / 󰷊 (djvu)

GhostBook         BookCard есть, файла НЕТ на диске. "Виртуальная" книга.
                  Метаданные заполнены (doi, arxiv, summary), файл не скачан.
                  Иконка: 󰈖 (dim, ghosted) + индикатор ○
                  Примеры: добавлено через arXiv ID без загрузки PDF

DetachedBook      BookCard есть, файл БЫЛ но ИСЧЕЗ (путь битый).
                  Иконка: 󰈖 + ⚠ (предупреждение)
                  Причины: переместили файл вручную, диск отключён
```

---

## 1. Архитектура данных

### 1.1 Структуры в `omniscope-core`

```rust
// omniscope-core/src/models/folder.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Folder {
    pub id: FolderId,              // UUID
    pub name: String,
    pub folder_type: FolderType,
    pub parent_id: Option<FolderId>,
    pub library_id: LibraryId,

    /// Только для PhysicalFolder: реальный путь на диске
    /// Относительный от корня библиотеки: "programming/rust"
    pub disk_path: Option<RelativePath>,

    /// Для VirtualFolder: иконка и цвет (пользовательские)
    pub icon: Option<String>,
    pub color: Option<String>,

    pub sort_order: SortOrder,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FolderType {
    Physical,      // Директория на диске
    Virtual,       // Только в метаданных
    LibraryRoot,   // Корень библиотеки
}

/// Статус присутствия файла книги на диске
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilePresence {
    /// Файл есть, путь актуален
    Present { path: AbsolutePath, size_bytes: u64, hash: Option<String> },
    /// Файл никогда не добавлялся (ghost book)
    NeverHadFile,
    /// Файл был, но исчез (detached book)
    Missing { last_known_path: AbsolutePath, last_seen: DateTime<Utc> },
}

/// Расширенный BookCard с информацией о файле
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCard {
    // ... существующие поля ...
    pub file_presence: FilePresence,
    pub folder_id: Option<FolderId>,      // Принадлежность к физической папке
    pub virtual_folder_ids: Vec<FolderId>, // Может быть в нескольких virtual folders
}
```

### 1.2 SQLite схема для папок

```sql
-- Таблица папок (физических и виртуальных)
CREATE TABLE folders (
    id          TEXT PRIMARY KEY,          -- UUID
    name        TEXT NOT NULL,
    folder_type TEXT NOT NULL CHECK(folder_type IN ('physical', 'virtual', 'library_root')),
    parent_id   TEXT REFERENCES folders(id) ON DELETE CASCADE,
    library_id  TEXT NOT NULL REFERENCES libraries(id),
    disk_path   TEXT,                      -- relative path, NULL для virtual
    icon        TEXT,
    color       TEXT,
    sort_index  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Связь книга ↔ виртуальная папка (M:N)
CREATE TABLE book_virtual_folders (
    book_id     TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    folder_id   TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    added_at    TEXT NOT NULL,
    PRIMARY KEY (book_id, folder_id)
);

-- Расширение таблицы books
ALTER TABLE books ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
ALTER TABLE books ADD COLUMN file_presence TEXT NOT NULL DEFAULT 'never_had_file';
ALTER TABLE books ADD COLUMN file_last_seen TEXT;

-- Индексы для производительности
CREATE INDEX idx_folders_parent ON folders(parent_id);
CREATE INDEX idx_folders_library ON folders(library_id);
CREATE INDEX idx_folders_disk_path ON folders(disk_path) WHERE disk_path IS NOT NULL;
CREATE INDEX idx_books_folder ON books(folder_id);
```

### 1.3 FolderTree — дерево в памяти

```rust
// omniscope-core/src/models/folder_tree.rs

/// In-memory дерево папок. Строится один раз при старте,
/// обновляется инкрементально при изменениях.
pub struct FolderTree {
    /// Корень библиотеки
    pub root: FolderNode,
    /// Быстрый доступ по ID
    pub index: HashMap<FolderId, *mut FolderNode>,
    /// Быстрый доступ по disk_path
    pub path_index: HashMap<RelativePath, FolderId>,
    pub library_id: LibraryId,
}

pub struct FolderNode {
    pub folder: Folder,
    pub children: Vec<FolderNode>,
    pub book_count: u32,          // включая подпапки
    pub book_count_direct: u32,   // только прямые книги
    pub ghost_count: u32,         // ghost books в поддереве
    pub is_expanded: bool,        // состояние UI
}

impl FolderTree {
    /// Построить из SQLite. Производительность: < 50ms для 1000 папок.
    pub async fn build(db: &Database, library_id: &LibraryId) -> Result<Self>;

    /// Найти папку по пути на диске (для sync)
    pub fn find_by_path(&self, path: &RelativePath) -> Option<&FolderNode>;

    /// Путь от корня до узла (хлебные крошки)
    pub fn breadcrumb(&self, folder_id: &FolderId) -> Vec<&Folder>;

    /// Все прямые дочерние папки
    pub fn children(&self, folder_id: &FolderId) -> &[FolderNode];

    /// Применить изменение (для инкрементального обновления)
    pub fn apply_change(&mut self, change: FolderTreeChange);
}

pub enum FolderTreeChange {
    FolderCreated(Folder),
    FolderRenamed { id: FolderId, new_name: String },
    FolderMoved { id: FolderId, new_parent_id: Option<FolderId> },
    FolderDeleted(FolderId),
    BookCountChanged { folder_id: FolderId, delta: i32 },
}
```

---

## 2. Двусторонняя синхронизация с диском

### 2.1 Принципы синхронизации

```
ДИСК → TUI (push):
  Новая директория обнаружена    → предложить создать папку в библиотеке
  Директория удалена             → папка становится "orphaned", книги detached
  Файл книги добавлен в папку   → auto-import или уведомление
  Файл книги удалён              → книга становится detached

TUI → ДИСК (pull):
  Пользователь создал папку      → mkdir на диске
  Пользователь переименовал      → mv директории на диске
  Пользователь переместил папку  → mv на диске
  Пользователь удалил папку      → rm -r на диске (только после подтверждения)
  Пользователь переместил книгу  → mv файла на диске
```

### 2.2 FolderSync — ядро синхронизации

```rust
// omniscope-core/src/sync/folder_sync.rs

pub struct FolderSync {
    library_root: PathBuf,
    db: Arc<Database>,
    tree: Arc<RwLock<FolderTree>>,
}

/// Результат сравнения диска и БД
pub struct SyncReport {
    /// Директории на диске, которых нет в БД
    pub untracked_dirs: Vec<PathBuf>,
    /// Папки в БД, директорий которых нет на диске
    pub orphaned_folders: Vec<FolderId>,
    /// Файлы книг на диске, которых нет в БД
    pub untracked_files: Vec<PathBuf>,
    /// Книги в БД, файлов которых нет на диске (становятся detached)
    pub missing_files: Vec<BookId>,
    /// Файлы, которые переместились (по hash)
    pub moved_files: Vec<(BookId, PathBuf)>,
}

impl FolderSync {
    /// Полное сканирование: сравнить диск и БД, вернуть список расхождений.
    /// Запускается при старте и по команде `omniscope sync`.
    /// НЕ применяет изменения автоматически — только репортит.
    pub async fn full_scan(&self) -> Result<SyncReport> {
        let disk_state = self.scan_disk().await?;
        let db_state = self.load_db_state().await?;
        Ok(self.diff(disk_state, db_state))
    }

    /// Применить синхронизацию с выбранной стратегией разрешения конфликтов.
    pub async fn apply_sync(
        &self,
        report: &SyncReport,
        strategy: SyncStrategy,
    ) -> Result<SyncApplyResult> {
        let mut result = SyncApplyResult::default();

        match strategy {
            SyncStrategy::DiskWins => {
                // Неотслеживаемые директории → создать папки в БД
                for dir in &report.untracked_dirs {
                    self.import_directory(dir).await?;
                    result.folders_created += 1;
                }
                // Осиротевшие папки → удалить из БД
                for folder_id in &report.orphaned_folders {
                    self.mark_folder_orphaned(folder_id).await?;
                    result.folders_orphaned += 1;
                }
                // Новые файлы → auto-import (создать карточку)
                for file in &report.untracked_files {
                    self.auto_import_file(file, strategy).await?;
                    result.books_imported += 1;
                }
            }
            SyncStrategy::Interactive => {
                // Вернуть отчёт — пользователь выбирает сам
                return Ok(SyncApplyResult { pending_review: Some(report.clone()), ..Default::default() });
            }
        }

        // Всегда: обновить file_presence для пропавших файлов
        for book_id in &report.missing_files {
            self.update_file_presence(book_id, FilePresence::Missing { .. }).await?;
        }

        // Всегда: обновить пути для переместившихся файлов
        for (book_id, new_path) in &report.moved_files {
            self.update_book_path(book_id, new_path).await?;
            result.books_relinked += 1;
        }

        Ok(result)
    }

    async fn scan_disk(&self) -> Result<DiskState> {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        // Обойти дерево директорий, игнорируя .libr/
        let mut stack = vec![self.library_root.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    let name = path.file_name().unwrap().to_str().unwrap();
                    // Игнорировать .libr/ и скрытые директории
                    if !name.starts_with('.') {
                        dirs.push(path.clone());
                        stack.push(path);
                    }
                } else if metadata.is_file() {
                    if Self::is_book_file(&path) {
                        files.push(path);
                    }
                }
            }
        }

        Ok(DiskState { dirs, files })
    }

    fn is_book_file(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("pdf" | "epub" | "djvu" | "fb2" | "mobi" | "azw3" | "cbz" | "cbr")
        )
    }

    async fn import_directory(&self, dir: &Path) -> Result<FolderId> {
        let relative = dir.strip_prefix(&self.library_root)?;
        let parent_path = relative.parent();
        let name = dir.file_name().unwrap().to_str().unwrap().to_string();

        // Найти родителя (или корень)
        let parent_id = if let Some(parent) = parent_path.filter(|p| !p.as_os_str().is_empty()) {
            let parent_rel = RelativePath::from(parent);
            self.tree.read().await.find_by_path(&parent_rel).map(|n| n.folder.id.clone())
        } else {
            None
        };

        let folder = Folder {
            id: Uuid::new_v4().to_string(),
            name,
            folder_type: FolderType::Physical,
            parent_id,
            disk_path: Some(RelativePath::from(relative)),
            ..Default::default()
        };

        self.db.create_folder(&folder).await?;
        self.tree.write().await.apply_change(FolderTreeChange::FolderCreated(folder.clone()));

        Ok(folder.id)
    }
}

#[derive(Debug, Clone)]
pub enum SyncStrategy {
    DiskWins,       // Диск всегда прав (тихая синхронизация)
    Interactive,    // Показать отчёт, пользователь выбирает
}
```

### 2.3 TUI → Диск: операции над папками

```rust
// omniscope-core/src/sync/folder_ops.rs

/// Все операции над папками — атомарны и с undo через ActionLog.
pub struct FolderOps {
    library_root: PathBuf,
    db: Arc<Database>,
    tree: Arc<RwLock<FolderTree>>,
    action_log: Arc<ActionLog>,
}

impl FolderOps {
    /// Создать папку. Параллельно: mkdir + запись в БД.
    pub async fn create_folder(
        &self,
        parent_id: Option<&FolderId>,
        name: &str,
        folder_type: FolderType,
    ) -> Result<Folder> {
        let parent_path = self.resolve_parent_path(parent_id).await?;
        let new_rel_path = parent_path.join(name);
        let new_abs_path = self.library_root.join(&new_rel_path);

        if matches!(folder_type, FolderType::Physical) {
            // Создать директорию на диске
            tokio::fs::create_dir_all(&new_abs_path).await
                .map_err(|e| FolderError::DiskError { path: new_abs_path.clone(), source: e })?;
        }

        let folder = Folder {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            folder_type,
            parent_id: parent_id.cloned(),
            disk_path: if matches!(folder_type, FolderType::Physical) {
                Some(RelativePath::from(new_rel_path))
            } else {
                None
            },
            ..Default::default()
        };

        self.db.create_folder(&folder).await?;
        self.tree.write().await.apply_change(FolderTreeChange::FolderCreated(folder.clone()));

        // Логировать для undo
        self.action_log.append(ActionLogEntry {
            action_type: "create_folder".to_string(),
            snapshot_before: None,
            snapshot_after: Some(serde_json::to_value(&folder)?),
            ..Default::default()
        }).await?;

        Ok(folder)
    }

    /// Переименовать папку. mv на диске + обновить БД.
    pub async fn rename_folder(
        &self,
        folder_id: &FolderId,
        new_name: &str,
    ) -> Result<()> {
        // Валидация имени
        validate_folder_name(new_name)?;

        let folder = self.db.get_folder(folder_id).await?;
        let snapshot_before = serde_json::to_value(&folder)?;

        if let Some(disk_path) = &folder.disk_path {
            let old_abs = self.library_root.join(disk_path.as_path());
            let new_abs = old_abs.parent().unwrap().join(new_name);

            // Проверить что нет конфликта имён
            ensure!(!new_abs.exists(), "Directory '{}' already exists", new_name);

            // mv на диске
            tokio::fs::rename(&old_abs, &new_abs).await?;

            // Обновить disk_path для папки и всех дочерних
            let new_rel = RelativePath::from(new_abs.strip_prefix(&self.library_root)?);
            self.db.update_folder_path_recursive(folder_id, disk_path, &new_rel).await?;
        }

        // Обновить имя в БД
        self.db.rename_folder(folder_id, new_name).await?;
        self.tree.write().await.apply_change(FolderTreeChange::FolderRenamed {
            id: folder_id.clone(),
            new_name: new_name.to_string(),
        });

        self.action_log.append(ActionLogEntry {
            action_type: "rename_folder".to_string(),
            snapshot_before: Some(snapshot_before),
            ..Default::default()
        }).await?;

        Ok(())
    }

    /// Переместить папку в другую папку. mv + обновить все дочерние пути.
    pub async fn move_folder(
        &self,
        folder_id: &FolderId,
        new_parent_id: Option<&FolderId>,
    ) -> Result<()> {
        let folder = self.db.get_folder(folder_id).await?;
        let snapshot = serde_json::to_value(&folder)?;

        if let Some(disk_path) = &folder.disk_path {
            let old_abs = self.library_root.join(disk_path.as_path());
            let new_parent_abs = if let Some(pid) = new_parent_id {
                let parent = self.db.get_folder(pid).await?;
                parent.disk_path
                    .map(|p| self.library_root.join(p.as_path()))
                    .unwrap_or(self.library_root.clone())
            } else {
                self.library_root.clone()
            };

            let new_abs = new_parent_abs.join(folder.name.clone());
            ensure!(!new_abs.exists(), "Destination '{}' already exists", folder.name);

            tokio::fs::rename(&old_abs, &new_abs).await?;

            // Обновить все disk_path в поддереве
            let new_rel = RelativePath::from(new_abs.strip_prefix(&self.library_root)?);
            let old_rel = disk_path.clone();
            self.db.update_folder_path_recursive(folder_id, &old_rel, &new_rel).await?;

            // Обновить пути файлов книг внутри папки
            self.db.update_book_paths_in_folder(folder_id, &old_rel, &new_rel).await?;
        }

        self.db.move_folder(folder_id, new_parent_id).await?;
        self.tree.write().await.apply_change(FolderTreeChange::FolderMoved {
            id: folder_id.clone(),
            new_parent_id: new_parent_id.cloned(),
        });

        self.action_log.append(ActionLogEntry {
            action_type: "move_folder".to_string(),
            snapshot_before: Some(snapshot),
            ..Default::default()
        }).await?;

        Ok(())
    }

    /// Удалить папку. Требует подтверждения если внутри есть книги.
    /// delete_mode: только папку, файлы остаются на диске (detach) / всё удалить
    pub async fn delete_folder(
        &self,
        folder_id: &FolderId,
        delete_mode: FolderDeleteMode,
    ) -> Result<DeleteFolderReport> {
        let folder = self.db.get_folder(folder_id).await?;
        let affected_books = self.db.count_books_in_subtree(folder_id).await?;

        let report = DeleteFolderReport {
            folder_name: folder.name.clone(),
            affected_books,
            will_delete_files: matches!(delete_mode, FolderDeleteMode::WithFiles),
        };

        if let Some(disk_path) = &folder.disk_path {
            let abs_path = self.library_root.join(disk_path.as_path());

            match delete_mode {
                FolderDeleteMode::KeepFiles => {
                    // Книги внутри становятся detached (привязаны к корню)
                    self.db.detach_books_from_folder(folder_id).await?;
                    // Удалить только директорию (она должна быть пустой или мы переместим файлы)
                    // Но раз файлы "остаются на диске" — мы их не трогаем, только убираем из иерархии
                    // Директория остаётся на диске (просто untracked)
                }
                FolderDeleteMode::WithFiles => {
                    // Удалить директорию со всем содержимым
                    tokio::fs::remove_dir_all(&abs_path).await?;
                }
            }
        }

        // Удалить папку из БД (cascade удалит дочерние папки)
        self.db.delete_folder(folder_id).await?;
        self.tree.write().await.apply_change(FolderTreeChange::FolderDeleted(folder_id.clone()));

        Ok(report)
    }

    /// Переместить книгу в папку.
    /// Физически перемещает файл книги + обновляет BookCard.
    pub async fn move_book_to_folder(
        &self,
        book_id: &BookId,
        target_folder_id: &FolderId,
    ) -> Result<()> {
        let book = self.db.get_book_card(book_id).await?;
        let target_folder = self.db.get_folder(target_folder_id).await?;

        // Для ghost books — только обновить метаданные, файл не трогаем
        if matches!(book.file_presence, FilePresence::NeverHadFile) {
            self.db.update_book_folder(book_id, Some(target_folder_id)).await?;
            return Ok(());
        }

        // Для физических книг — переместить файл
        if let FilePresence::Present { path, .. } = &book.file_presence {
            if let Some(target_disk_path) = &target_folder.disk_path {
                let target_dir = self.library_root.join(target_disk_path.as_path());
                let file_name = path.file_name().unwrap();
                let new_file_path = target_dir.join(file_name);

                // Избежать конфликта имён
                let final_path = self.resolve_name_conflict(&new_file_path).await;

                tokio::fs::rename(path.as_path(), &final_path).await?;

                // Обновить путь в карточке
                self.db.update_book_file_path(book_id, &final_path).await?;
            }
        }

        self.db.update_book_folder(book_id, Some(target_folder_id)).await?;
        self.tree.write().await.apply_change(FolderTreeChange::BookCountChanged {
            folder_id: target_folder_id.clone(),
            delta: 1,
        });

        Ok(())
    }
}

pub enum FolderDeleteMode {
    KeepFiles,   // Удалить папку из библиотеки, файлы на диске не трогать
    WithFiles,   // Удалить папку + все файлы книг внутри
}

fn validate_folder_name(name: &str) -> Result<()> {
    ensure!(!name.is_empty(), "Folder name cannot be empty");
    ensure!(name.len() <= 255, "Folder name too long");
    ensure!(!name.contains('/'), "Folder name cannot contain '/'");
    ensure!(!name.contains('\0'), "Folder name cannot contain null bytes");
    ensure!(!matches!(name, "." | ".."), "Invalid folder name");
    Ok(())
}
```

---

## 3. Автосканирование — Filesystem Watcher

### 3.1 Архитектура вотчера

```rust
// omniscope-core/src/sync/watcher.rs

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use tokio::sync::mpsc;

pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,    // notify watcher (inotify/kqueue/FSEvents)
    event_rx: mpsc::Receiver<WatcherEvent>,
    config: WatcherConfig,
    folder_sync: Arc<FolderSync>,
    debouncer: EventDebouncer,
}

#[derive(Debug, Clone)]
pub enum WatcherEvent {
    /// Новый файл книги обнаружен
    NewBookFile { path: PathBuf },
    /// Файл книги удалён
    BookFileRemoved { path: PathBuf },
    /// Файл книги переименован
    BookFileRenamed { from: PathBuf, to: PathBuf },
    /// Новая директория создана
    DirectoryCreated { path: PathBuf },
    /// Директория удалена
    DirectoryRemoved { path: PathBuf },
    /// Директория переименована
    DirectoryRenamed { from: PathBuf, to: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    /// Автоматически создавать карточку для новых файлов без подтверждения
    pub auto_import: bool,
    /// Пауза перед обработкой события (debounce) — ждём завершения копирования
    pub debounce_ms: u64,
    /// Минимальный размер файла для auto-import (избегать частичных загрузок)
    pub min_file_size_bytes: u64,
    /// Расширения для отслеживания
    pub watch_extensions: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            auto_import: false,        // По умолчанию: только уведомлять
            debounce_ms: 2000,         // 2 секунды дебаунса
            min_file_size_bytes: 1024, // Минимум 1KB
            watch_extensions: vec![
                "pdf".into(), "epub".into(), "djvu".into(),
                "fb2".into(), "mobi".into(), "azw3".into(),
            ],
        }
    }
}

impl LibraryWatcher {
    pub fn start(
        library_root: PathBuf,
        folder_sync: Arc<FolderSync>,
        config: WatcherConfig,
    ) -> Result<Self> {
        let (raw_tx, raw_rx) = mpsc::channel(256);
        let (event_tx, event_rx) = mpsc::channel(64);

        // notify вотчер — использует inotify (Linux), kqueue (macOS), ReadDirectoryChangesW (Windows)
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = raw_tx.blocking_send(event);
            }
        })?;

        // Рекурсивно отслеживать корень библиотеки, исключая .libr/
        watcher.watch(&library_root, RecursiveMode::Recursive)?;

        // Запустить задачу обработки событий
        let config_clone = config.clone();
        let root_clone = library_root.clone();
        tokio::spawn(async move {
            Self::process_events(raw_rx, event_tx, root_clone, config_clone).await;
        });

        Ok(Self {
            _watcher: watcher,
            event_rx,
            config,
            folder_sync,
            debouncer: EventDebouncer::new(config.debounce_ms),
        })
    }

    async fn process_events(
        mut raw_rx: mpsc::Receiver<Event>,
        event_tx: mpsc::Sender<WatcherEvent>,
        library_root: PathBuf,
        config: WatcherConfig,
    ) {
        // Буфер для дебаунса rename событий (Create + Remove = Rename)
        let mut pending_removes: HashMap<PathBuf, tokio::time::Instant> = HashMap::new();

        while let Some(event) = raw_rx.recv().await {
            // Игнорировать события в .libr/
            if event.paths.iter().any(|p| p.starts_with(library_root.join(".libr"))) {
                continue;
            }

            match event.kind {
                EventKind::Create(kind) => {
                    for path in &event.paths {
                        if path.is_dir() {
                            let _ = event_tx.send(WatcherEvent::DirectoryCreated {
                                path: path.clone()
                            }).await;
                        } else if Self::is_book_extension(path, &config.watch_extensions) {
                            // Дождаться завершения записи файла
                            let path = path.clone();
                            let tx = event_tx.clone();
                            let min_size = config.min_file_size_bytes;
                            let debounce = config.debounce_ms;

                            tokio::spawn(async move {
                                // Подождать debouncems и проверить размер
                                tokio::time::sleep(Duration::from_millis(debounce)).await;

                                if let Ok(meta) = tokio::fs::metadata(&path).await {
                                    if meta.len() >= min_size {
                                        let _ = tx.send(WatcherEvent::NewBookFile { path }).await;
                                    }
                                }
                            });
                        }
                    }
                }
                EventKind::Remove(_) => {
                    for path in &event.paths {
                        pending_removes.insert(path.clone(), tokio::time::Instant::now());
                    }
                }
                EventKind::Modify(_) => {
                    // Файл мог переименоваться: Remove + Create в окне дебаунса = Rename
                    // Логика сопоставления через inode (на Linux) или временны́е метки
                }
                _ => {}
            }

            // Обработать накопленные Remove — те что не были частью Rename
            let now = tokio::time::Instant::now();
            pending_removes.retain(|path, &mut ts| {
                if now.duration_since(ts).as_millis() > config.debounce_ms as u128 {
                    if path.is_dir() {
                        let _ = event_tx.blocking_send(WatcherEvent::DirectoryRemoved {
                            path: path.clone()
                        });
                    } else {
                        let _ = event_tx.blocking_send(WatcherEvent::BookFileRemoved {
                            path: path.clone()
                        });
                    }
                    false
                } else {
                    true
                }
            });
        }
    }

    /// Главный цикл обработки — вызывается из TUI event loop
    pub async fn handle_next_event(&mut self) -> Option<WatcherAction> {
        let event = self.event_rx.try_recv().ok()?;

        match event {
            WatcherEvent::NewBookFile { path } => {
                if self.config.auto_import {
                    // Тихий auto-import
                    Some(WatcherAction::AutoImport { path })
                } else {
                    // Показать уведомление в TUI
                    Some(WatcherAction::NotifyNewFile { path })
                }
            }
            WatcherEvent::BookFileRemoved { path } => {
                // Найти книгу по пути и обновить file_presence
                Some(WatcherAction::MarkFileDetached { path })
            }
            WatcherEvent::DirectoryCreated { path } => {
                // Предложить создать папку или синхронизировать тихо
                Some(WatcherAction::SyncNewDirectory { path })
            }
            WatcherEvent::DirectoryRemoved { path } => {
                Some(WatcherAction::MarkFolderOrphaned { path })
            }
            WatcherEvent::BookFileRenamed { from, to } => {
                Some(WatcherAction::UpdateBookPath { old_path: from, new_path: to })
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum WatcherAction {
    AutoImport { path: PathBuf },
    NotifyNewFile { path: PathBuf },
    MarkFileDetached { path: PathBuf },
    SyncNewDirectory { path: PathBuf },
    MarkFolderOrphaned { path: PathBuf },
    UpdateBookPath { old_path: PathBuf, new_path: PathBuf },
}
```

### 3.2 Уведомления о новых файлах в TUI

При обнаружении нового файла (если `auto_import = false`) в статус-баре появляется
неинвазивное уведомление:

```
╔══════════════════════════════════════════════════════════════════════════╗
║  [N]  programming/rust/    147 books                      [+1 new file] ║  ← мигает 3с
╚══════════════════════════════════════════════════════════════════════════╝

Горячая клавиша для обработки:
  Space     Перейти к уведомлению (сфокусироваться на новом файле)
  :import   Открыть панель импорта
```

Панель уведомлений о новых файлах (`:import new` или по уведомлению):

```
╭─ NEW FILES DETECTED ──────────────────────────────────────────────────╮
│  3 new files found in ~/Books/programming/rust/                        │
│                                                                         │
│  [󰄬] async-rust-oreilly.pdf    (2.4 MB)  → programming/rust/          │
│  [󰄬] tokio-guide.epub           (890 KB)  → programming/rust/async/   │
│  [ ] unknown-book.pdf            (5.1 MB)  → programming/              │
│       ↳ Could not extract metadata                                     │
│                                                                         │
│  [a] Import all  [s] Import selected  [e] Edit metadata  [Esc] Skip   │
╰─────────────────────────────────────────────────────────────────────────╯
```

---

## 4. Левая панель — Folder Tree Mode

### 4.1 Два режима левой панели

Левая панель переключается между двумя режимами через `Tab` (когда панель активна)
или через `gl` из Normal mode.

```
РЕЖИМ 1: LIBRARY VIEW (текущий, по умолчанию)
──────────────────────────────────────────────
╭─ LIBRARIES ─────────────╮
│ 󰂺  All Books      147    │
│ ▾ 󰂺  Programming   89    │  ← библиотеки
│   ├ 󰝰  rust         34   │  ← физические папки как плоский список
│   ├ 󰝰  algorithms   12   │
│   └ 󰝰  systems      43   │
│ ▸ 󰂺  ML Papers      38   │
│                          │
│ ─── TAGS ─────────────  │
│  󰌒 rust            34   │
│  󰌒 async           18   │
╰──────────────────────────╯

РЕЖИМ 2: FOLDER TREE VIEW (новый)
──────────────────────────────────
╭─ FOLDERS ────────────────╮
│ 󰝰  ~/Books/         147  │  ← корень (library root)
│ ▾ 󰝰  programming/    89  │  ← физическая папка
│   ▾ 󰝰  rust/         34  │
│     ├ 󰝰  official/    5  │
│     ├ 󰝰  community/  12  │
│     └ 󰉋  exercises/  17  │  ← свёрнута
│   ├ 󰝰  algorithms/   12  │
│   └ 󰝰  systems/      43  │
│ ▾ 󰝰  ml-papers/      38  │
│   ├ 󰝰  transformers/ 12  │
│   └ 󰝰  rl/           8   │
│ 󰉋  fiction/          31  │  ← свёрнута, click → expand
│                           │
│ ─── VIRTUAL ───────────  │
│ ⊕ Thesis List        15  │  ← виртуальная папка
│ ⊕ Favorites           8  │
│ ⊕ Read in 2025       22  │
│                           │
│  [+] New folder           │
╰───────────────────────────╯
```

### 4.2 Визуальные индикаторы

```
Иконки папок:
  󰝰   — PhysicalFolder, развёрнута
  󰉋   — PhysicalFolder, свёрнута
  ⊕   — VirtualFolder
  󰝰   — LibraryRoot (с подчёркиванием или другим цветом)

Иконки книг (в счётчиках и Folder View):
  󰈙   — PDF
  󰃴   — EPUB
  󰷊   — DjVu
  󰈖   — Ghost book (dim, nord3)
  ⚠   — Detached book (оранжевый, nord12)

Цвета счётчиков:
  "34"    — nord3 (normal)
  "34+2○" — '34' nord3, '+2' nord12, '○' = ghost books
           (означает: 34 физических + 2 ghost)

Дерево:
  ▾  — развёрнутый узел (nord8)
  ▸  — свёрнутый узел (nord3)
  ├─ — средний потомок (nord2)
  └─ — последний потомок (nord2)

Строка с ghost books:
  Полупрозрачный вид: nord3 вместо nord4/nord5
  Тег ○ в конце: "programming/   34+2○"
```

### 4.3 Рендеринг левой панели (Folder Tree Mode)

```rust
// omniscope-tui/src/panels/left/folder_tree.rs

pub struct FolderTreePanel {
    pub tree: Arc<RwLock<FolderTree>>,
    pub cursor: FolderTreeCursor,
    pub mode: LeftPanelMode,
    pub expand_state: HashMap<FolderId, bool>,
    pub virtual_section_visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LeftPanelMode {
    LibraryView,   // Режим библиотек + тегов
    FolderTree,    // Режим дерева папок
}

pub struct FolderTreeCursor {
    pub selected_id: Option<FolderId>,
    pub visual_line: usize,     // Строка на экране (для рендера)
    pub scroll_offset: usize,
}

impl FolderTreePanel {
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_active: bool) {
        let title = match self.mode {
            LeftPanelMode::LibraryView => " LIBRARIES ",
            LeftPanelMode::FolderTree  => " FOLDERS ",
        };

        // Граница панели
        let border_style = if is_active {
            Style::default().fg(theme.frost_ice)
        } else {
            Style::default().fg(theme.border)
        };
        frame.render_widget(
            Block::default().title(title).borders(Borders::ALL).border_style(border_style),
            area,
        );

        let inner = area.inner(&Margin { horizontal: 1, vertical: 1 });

        // Виртуализированный список узлов
        let flat = self.flatten_tree();
        let visible: Vec<_> = flat.iter()
            .skip(self.cursor.scroll_offset)
            .take(inner.height as usize)
            .collect();

        for (i, node) in visible.iter().enumerate() {
            let y = inner.y + i as u16;
            self.render_folder_row(frame, node, inner.x, y, inner.width, theme);
        }
    }

    fn render_folder_row(
        &self,
        frame: &mut Frame,
        node: &FlatFolderNode,
        x: u16, y: u16, width: u16,
        theme: &Theme,
    ) {
        let is_selected = self.cursor.selected_id.as_deref() == Some(&node.folder.id);

        // Отступ по глубине
        let indent = "  ".repeat(node.depth);

        // Символ раскрытия
        let expand_sym = if node.has_children {
            if node.is_expanded { "▾ " } else { "▸ " }
        } else {
            "  "
        };

        // Иконка
        let icon = match node.folder.folder_type {
            FolderType::Physical => {
                if node.is_expanded { "󰝰 " } else { "󰉋 " }
            }
            FolderType::Virtual => "⊕ ",
            FolderType::LibraryRoot => "󰝰 ",
        };

        // Имя + счётчик
        let name_style = if is_selected {
            Style::default().fg(theme.fg_bright).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };

        let count_str = if node.ghost_count > 0 {
            format!("{:>3}+{}○", node.book_count - node.ghost_count, node.ghost_count)
        } else {
            format!("{:>3}", node.book_count)
        };

        // Фон для выбранной строки
        let bg = if is_selected {
            Style::default().bg(theme.bg_secondary)
        } else {
            Style::default()
        };

        // Собрать строку
        let line = Line::from(vec![
            Span::raw(format!("{}{}{}", indent, expand_sym, icon)),
            Span::styled(node.folder.name.clone(), name_style),
            Span::styled(format!("  {}", count_str), Style::default().fg(theme.muted)),
        ]);

        frame.render_widget(
            Paragraph::new(line).style(bg),
            Rect { x, y, width, height: 1 },
        );
    }

    /// Развернуть дерево в плоский список для рендера
    fn flatten_tree(&self) -> Vec<FlatFolderNode> {
        let tree = self.tree.blocking_read();
        let mut result = Vec::new();
        self.flatten_node(&tree.root, 0, &mut result);
        result
    }

    fn flatten_node(&self, node: &FolderNode, depth: usize, result: &mut Vec<FlatFolderNode>) {
        let is_expanded = *self.expand_state.get(&node.folder.id).unwrap_or(&false);
        result.push(FlatFolderNode {
            folder: node.folder.clone(),
            depth,
            is_expanded,
            has_children: !node.children.is_empty(),
            book_count: node.book_count,
            ghost_count: node.ghost_count,
        });
        if is_expanded {
            for child in &node.children {
                self.flatten_node(child, depth + 1, result);
            }
        }
    }
}
```

---

## 5. FOLDER Mode — vim-режим для операций над папками

### 5.1 Когда активируется

```
FOLDER mode активируется когда:
  • Фокус находится в левой панели (Folder Tree mode) — автоматически
  • Нажата клавиша `gF` из Normal mode при активной левой панели
  • Нажата `-` находясь в Folder View центральной панели (перейти в родителя)

FOLDER mode деактивируется:
  • `Esc` → возврат в Normal mode, фокус на центральной панели
  • `l` или `Enter` → перейти в папку (центральная панель показывает содержимое)
  • `h` → перейти к родительской папке
```

### 5.2 Полная карта клавиш FOLDER mode

```
╔══════════════════════════════════════════════════════════════════════╗
║  FOLDER MODE — управление деревом папок                              ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                       ║
║  НАВИГАЦИЯ ─────────────────────────────────────────────────────────║
║  j / k         Следующая / предыдущая папка                          ║
║  gg / G        Первая / последняя папка                              ║
║  h             Свернуть папку / перейти к родителю                   ║
║  l / Enter     Развернуть папку / войти (показать в центре)          ║
║  za / zo / zc  Toggle fold / открыть / закрыть                       ║
║  zR / zM       Развернуть всё / свернуть всё                         ║
║  {count}j/k    Прыжки на N папок                                     ║
║  Ctrl+f/b      Прокрутка страницами                                  ║
║  /             Поиск по имени папки (вперёд)                         ║
║  ?             Поиск назад                                            ║
║  n / N         Следующий / предыдущий результат поиска               ║
║                                                                       ║
║  СОЗДАНИЕ ──────────────────────────────────────────────────────────║
║  a / gcf       Создать дочернюю Physical папку (ввод имени)          ║
║  A             Создать дочернюю Virtual папку                        ║
║  O             Создать папку-соседа (на том же уровне, выше)         ║
║  o             Создать папку-соседа (на том же уровне, ниже)         ║
║                                                                       ║
║  ИЗМЕНЕНИЕ ─────────────────────────────────────────────────────────║
║  r             Переименовать текущую папку (inline editing)           ║
║  R             Переименовать с preview зависимостей                  ║
║  I             Изменить иконку (для VirtualFolder)                   ║
║  c             Изменить цвет метки (для VirtualFolder)               ║
║                                                                       ║
║  УДАЛЕНИЕ ──────────────────────────────────────────────────────────║
║  dd            Удалить папку (книги становятся detached)             ║
║  dD            Удалить папку + все файлы (с двойным подтверждением)  ║
║  d_            Удалить папку из библиотеки, оставив на диске         ║
║                                                                       ║
║  ПЕРЕМЕЩЕНИЕ ───────────────────────────────────────────────────────║
║  m{a-z}        Отметить папку в регистр {a-z}                        ║
║  p             Вставить (переместить) из регистра                    ║
║  P             Скопировать структуру (только virtual folders)        ║
║  J             Переместить текущую папку вниз среди соседей          ║
║  K             Переместить текущую папку вверх среди соседей         ║
║  >             Сделать дочерней для предыдущего соседа               ║
║  <             Поднять на уровень выше (стать соседом родителя)      ║
║                                                                       ║
║  ВЫДЕЛЕНИЕ ─────────────────────────────────────────────────────────║
║  v / V         Visual mode (выбор нескольких папок)                  ║
║  Tab           Пометить папку для пакетной операции (в visual)       ║
║                                                                       ║
║  ДЕЙСТВИЯ ──────────────────────────────────────────────────────────║
║  Enter / l     Открыть папку (центр показывает содержимое)           ║
║  Space         Toggle expand/collapse                                 ║
║  T             Сортировка: имя / count / дата / кастом               ║
║  s             Переключить: Physical/Virtual view                    ║
║  Tab           Переключить режим панели (LibraryView ↔ FolderTree)  ║
║  Esc           Выйти в Normal mode (фокус → центральная панель)      ║
║                                                                       ║
║  ИНФОРМАЦИЯ ────────────────────────────────────────────────────────║
║  ?             Показать справку (этот экран)                          ║
║  gi            Информация о папке (статистика, путь)                  ║
║  gd            Перейти к папке на диске в $FILEMAN                   ║
║  yy            Скопировать путь папки в clipboard                    ║
║  yp            Скопировать полный путь (absolute)                    ║
╚══════════════════════════════════════════════════════════════════════╝
```

### 5.3 Inline Rename (клавиша `r`)

```
Поведение inline rename — аналог netrw rename в Vim:

До:
  ▾ 󰝰  rust/         34

После нажатия r: курсор на имени, режим INPUT
  ▾ 󰝰  [rust           ]  34   ← поле ввода, выделен текущий текст

Клавиши в режиме rename:
  Enter   Применить
  Esc     Отмена
  Ctrl+a  Выбрать всё
  Ctrl+u  Очистить
  Tab     Автодополнение (другие папки на том же уровне)

После Enter: mv на диске + обновление БД
             Показать: "Renamed: 'rust/' → 'rust-lang/'" в статус-баре
```

### 5.4 Создание папки (`a`)

```
Нажать a → появляется строка ввода под текущей папкой:

  ▾ 󰝰  programming/   89
    ▾ 󰝰  rust/         34
      ├ 󰝰  official/    5
      ├ 󰝰  community/  12
      └ [              ]   ← курсор здесь, пустое поле

Подсказки:
  • Ввести "async" → создаст programming/rust/async/
  • Ввести "theory/advanced" → создаст вложенную структуру
  • Префикс "~" или "/" → предупреждение: используй относительные пути

После Enter:
  1. mkdir programming/rust/async/ на диске
  2. Создать запись в БД
  3. Развернуть родителя
  4. Курсор на новой папке
  5. Статус: "Created: programming/rust/async/"
```

### 5.5 Visual mode в FOLDER mode

```
V (Visual Line) — выбор нескольких папок:

  ▾ 󰝰  programming/   89   ← visual start
  ▸ 󰝰  ml-papers/     38   ← highlight
  ▸ 󰝰  fiction/        31   ← visual end (курсор)

Операторы в visual:
  d     Удалить все выбранные (с подтверждением)
  y     Скопировать пути
  m     Переместить все выбранные в новую родительскую папку
  >     Сделать все дочерними для первой из выбранных (merge)
  @t    Запустить AI тегирование для всех книг в выбранных папках
```

---

## 6. Центральная панель — Folder View Mode

### 6.1 Два режима центральной панели

```
РЕЖИМ 1: BOOK LIST MODE (текущий, по умолчанию)
 Отображает: список книг из выбранной библиотеки/папки/тега
 Сортировка: по title/author/year/rating/frecency

РЕЖИМ 2: FOLDER VIEW MODE (новый)
 Отображает: содержимое текущей директории (папки + книги вперемешку)
 Как файловый менеджер, но с книжными метаданными
 Переключение: gv из Normal mode, или Enter по папке в центральной панели
```

### 6.2 Внешний вид Folder View

```
╔══════════════════════════════════════════════════════════════════════╗
║  [N]  📂 programming / rust / official                               ║  ← breadcrumb
╠══════════════════════════════════════════════════════════════════════╣
║          ║  󰂺 official/  (5 книг)               [FOLDER VIEW]  Nord ║
╠══════════════════════════════════════════════════════════════════════╣
│          ║                                                            │
│          ║  ▸ FOLDERS (2)  ──────────────────────────────────────── │
│          ║                                                            │
│          ║   󰉋  exercises/                              8 книг      │
│          ║   󰉋  examples/                               3 книги     │
│          ║                                                            │
│          ║  ▸ BOOKS (5)  ─────────────────────────────────────────  │
│          ║                                                            │
│          ║  ▶ 󰈙  The Rust Programming Language        Klabnik 2023  │  ← cursor
│          ║     ★★★★★  ✓ read   [rust] [official] [beginner]         │
│          ║                                                            │
│          ║     󰈙  Rust Reference Manual               Rust Team      │
│          ║     ★★★★☆  ○ unread  [reference] [official]               │
│          ║                                                            │
│          ║     󰈙  Rustonomicon                        Rust Team      │
│          ║     ★★★★★  ● reading  [unsafe] [advanced]                 │
│          ║                                                            │
│          ║     󰈖  Rust API Guidelines                 Rust Team      │  ← Ghost book
│          ║     ○ ghost • no file    [official] [reference]           │  ← dim style
│          ║                                                            │
│          ║     󰈙  Rust by Example                    Rust Team      │
│          ║     ★★★☆☆  ✓ read   [examples] [beginner]                │
│          ║                                                            │
╠══════════════════════════════════════════════════════════════════════╣
║  FOLDER  path:programming/rust/official   5 books (1 ghost) ● rust  ║
╚══════════════════════════════════════════════════════════════════════╝
```

### 6.3 Навигация в Folder View

```
В FOLDER VIEW MODE, Normal mode клавиши:

j / k         Следующий / предыдущий элемент (папки + книги)
Enter         Войти в папку / Открыть книгу (двойное действие)
-             Выйти в родительскую папку (аналог vim netrw)
l             Войти в папку (если курсор на папке)
h             Выйти в родителя
gg / G        Первый / последний элемент
Ctrl+f/b      Прокрутка

gv            Переключить Folder View ↔ Book List
gb            Перейти в левую панель (выбрать другую папку)

Операторы работают как в Book List:
  dd          Удалить (папку или книгу — в зависимости от позиции курсора)
  yy          Скопировать (путь папки или заголовок книги)
  p           Вставить (переместить)
  r           Переименовать
  a           Создать новую папку здесь

Поиск работает внутри текущей папки и поддиректорий:
  /           Поиск (fuzzy по имени папки / заголовку книги)
  :g/...      Глобальная команда по паттерну

Text objects специфичные для Folder View:
  if / af     Inner/around folder (все книги в папке)
  ib / ab     Inner/around book (одна книга)
```

### 6.4 Хлебные крошки (Breadcrumb)

```rust
// omniscope-tui/src/panels/center/breadcrumb.rs

pub fn render_breadcrumb(
    frame: &mut Frame,
    area: Rect,
    path: &[&Folder],
    theme: &Theme,
) {
    // Путь: programming / rust / official
    // Каждый сегмент кликабелен (или навигируем через h)

    let mut spans = Vec::new();
    for (i, folder) in path.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" / ", Style::default().fg(theme.muted)));
        }

        let is_last = i == path.len() - 1;
        let style = if is_last {
            Style::default().fg(theme.fg_bright).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.frost_mint)
        };

        spans.push(Span::styled(folder.name.clone(), style));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        area,
    );
}
```

### 6.5 Ghost books в Folder View

Ghost books (книги без файла) отображаются в dimmed стиле:

```
Внешний вид строки ghost book:
  󰈖  Rust API Guidelines      Rust Team     ← nord3 (dim) для всей строки
  ○ ghost • no file  [reference]            ← "○ ghost" nord12 (orange), остальное nord3

Доступные операции над ghost book:
  Enter         Открыть карточку (метаданные доступны)
  @m            Обогатить метаданные через CrossRef/arXiv
  gf            Попытаться найти файл (Sci-Hub / Anna's Archive)
  E             Прикрепить существующий файл (выбор файла)
  dd            Удалить карточку (файл не трогается — его нет)

НЕ доступно:
  o / O         Открыть в внешнем приложении (нет файла)

При наведении (preview в правой панели):
  Показать карточку с метаданными + заметку "File not available"
  + кнопка быстрого действия [Find PDF?]
```

---

## 7. Sync Panel — панель синхронизации

Вызывается через `:sync`, `@sync`, или автоматически при обнаружении расхождений.

```
╔══════════════════════════════════════════════════════════════════════╗
║  SYNC STATUS                                             omniscope   ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                       ║
║  Library: ~/Books/   Last scan: 2 min ago                            ║
║                                                                       ║
║  ⊕ NEW (3) ────────────────────────────────────────────────────────  ║
║  Files on disk, no card in library:                                  ║
║                                                                       ║
║  [󰄬] programming/rust/async-rust-oreilly.pdf    (2.4 MB)            ║
║       Detected metadata: "Async Rust" · Oreilly · 2024               ║
║  [󰄬] ml-papers/attention-v6.pdf                 (1.1 MB)            ║
║       arXiv: 1706.03762v6 (update to existing card)                  ║
║  [ ] unknown/mystery.pdf                        (340 KB)             ║
║       No metadata found — needs manual review                        ║
║                                                                       ║
║  ⚠ DETACHED (2) ───────────────────────────────────────────────────  ║
║  Cards exist, files missing:                                         ║
║                                                                       ║
║  [󰈖] "Programming Rust 2ed"  → programming/rust/programming-rust.pdf║
║       Last seen: 3 days ago. [Locate] [Relink] [Keep as ghost]       ║
║  [󰈖] "TAOCP Vol.1"           → archive/taocp-v1.pdf                 ║
║       [Locate] [Relink] [Keep as ghost]                              ║
║                                                                       ║
║  󰉋 UNTRACKED DIRS (1) ─────────────────────────────────────────────  ║
║  Directories on disk, not in library:                                 ║
║                                                                       ║
║  [ ] programming/new-stuff/                     (7 files)            ║
║      [Import folder] [Ignore]                                        ║
║                                                                       ║
╠══════════════════════════════════════════════════════════════════════╣
║  [a] Apply all  [s] Apply selected  [i] Ignore  [r] Re-scan  [Esc]  ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## 8. CLI команды для папочной системы

```bash
# ── УПРАВЛЕНИЕ ПАПКАМИ ─────────────────────────────────────────────────

# Создать физическую папку
omniscope folder create programming/rust/async --json
# → {"status":"ok","folder":{"id":"...","name":"async","disk_path":"programming/rust/async"}}

# Создать виртуальную папку
omniscope folder create "Thesis 2025" --virtual --json

# Переименовать
omniscope folder rename programming/rust "rust-lang" --json
omniscope folder rename {folder_id} "new-name" --by-id --json

# Переместить
omniscope folder move programming/rust ml-papers/ --json
omniscope folder move {folder_id} --into {parent_folder_id} --by-id --json

# Удалить (только папку, книги detach)
omniscope folder delete programming/old/ --json
# Удалить вместе с файлами (требует --confirm)
omniscope folder delete programming/old/ --with-files --confirm --json

# Показать дерево
omniscope folder tree --json
omniscope folder tree programming/ --depth 3 --json

# Информация о папке
omniscope folder info programming/rust/ --json
# → {"folder":{...}, "book_count":34, "ghost_count":2, "disk_size_mb":145}

# ── СИНХРОНИЗАЦИЯ ──────────────────────────────────────────────────────

# Полное сканирование (только репорт, без изменений)
omniscope sync --dry-run --json

# Синхронизировать (диск побеждает)
omniscope sync --strategy disk-wins --json

# Только обновить статус файлов (detached/present)
omniscope sync --check-files-only --json

# Сканировать конкретную директорию
omniscope scan programming/new-stuff/ --auto-import --json

# Включить/выключить watcher
omniscope watch start --json
omniscope watch stop --json
omniscope watch status --json

# ── GHOST BOOKS ────────────────────────────────────────────────────────

# Показать все ghost books
omniscope book list --ghost --json
omniscope book list --detached --json

# Попытаться найти файл для ghost book
omniscope book locate {book_id} --sources "annas,sci-hub,unpaywall" --json

# Прикрепить файл к ghost book
omniscope book attach {book_id} --file /path/to/file.pdf --json

# Конвертировать detached в ghost (убрать путь)
omniscope book detach {book_id} --json

# ── ПЕРЕМЕЩЕНИЕ КНИГ ───────────────────────────────────────────────────

# Переместить книгу в папку (физически перемещает файл)
omniscope book move {book_id} --folder programming/rust/async --json

# Переместить несколько
omniscope book move {id1} {id2} {id3} --folder ml-papers/ --json

# Добавить в виртуальную папку (файл не перемещается)
omniscope book virtual-add {book_id} --virtual-folder "Thesis 2025" --json
omniscope book virtual-remove {book_id} --virtual-folder "Thesis 2025" --json
```

---

## 9. Конфигурация

```toml
# .libr/library.toml (настройки конкретной библиотеки)

[folders]
# Как создавать папки: manual (только вручную) | auto_from_disk (при sync)
creation_mode = "manual"

# Автоматически создавать папки в БД при обнаружении директорий на диске
auto_sync_dirs = true

# Паттерны игнорирования (glob)
ignore_patterns = [
    ".DS_Store",
    "Thumbs.db",
    "__pycache__",
    "*.tmp",
]

[watcher]
# Отслеживать изменения на диске в реальном времени
enabled = true
debounce_ms = 2000
# Автоматически импортировать новые файлы без подтверждения
auto_import = false
min_file_size_kb = 10
# Расширения для отслеживания
extensions = ["pdf", "epub", "djvu", "fb2", "mobi", "azw3", "cbz"]

[sync]
# Стратегия по умолчанию при конфликтах
default_strategy = "interactive"   # interactive | disk_wins | library_wins
# Проверять целостность файлов при sync (slow but safe)
verify_file_hashes = false
# Как часто автоматически запускать sync
auto_sync_interval_minutes = 0    # 0 = только вручную

[folder_view]
# Показывать ghost books в Folder View
show_ghost_books = true
# Стиль отображения ghost books
ghost_style = "dimmed"    # dimmed | strikethrough | labeled

# Сортировка в Folder View: folders_first | mixed | books_first
entry_order = "folders_first"

[left_panel]
# Начальный режим левой панели
default_mode = "library_view"    # library_view | folder_tree
# Показывать счётчик ghost books в дереве папок
show_ghost_count = true
# Показывать VirtualFolder секцию
show_virtual_folders = true
```

---

## 10. Структуры состояния TUI

```rust
// omniscope-tui/src/state/folder_state.rs

/// Полное состояние подсистемы папок в TUI.
/// Является частью глобального AppState.
pub struct FolderState {
    // ── Левая панель ─────────────────────────────────────────────
    pub left_mode: LeftPanelMode,
    pub folder_tree: Arc<RwLock<FolderTree>>,
    pub tree_cursor: FolderTreeCursor,
    pub tree_expand_state: HashMap<FolderId, bool>,

    // ── Центральная панель ────────────────────────────────────────
    pub center_mode: CenterPanelMode,
    /// Текущая папка в Folder View
    pub current_folder: Option<FolderId>,
    /// Путь (хлебные крошки) в Folder View
    pub breadcrumb: Vec<Folder>,
    /// Содержимое текущей папки (папки + книги)
    pub folder_contents: FolderContents,

    // ── Watcher ───────────────────────────────────────────────────
    pub pending_watcher_events: VecDeque<WatcherAction>,

    // ── Операции ──────────────────────────────────────────────────
    /// Активная rename операция
    pub rename_state: Option<RenameState>,
    /// Активный sync report (если открыта Sync Panel)
    pub sync_report: Option<SyncReport>,
    pub sync_panel_visible: bool,

    // ── Drag state (для mouse поддержки) ──────────────────────────
    pub drag_source: Option<FolderDragSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CenterPanelMode {
    BookList,     // Список книг (текущий режим)
    FolderView,   // Просмотр содержимого папки
}

pub struct FolderContents {
    pub subfolders: Vec<FolderNode>,
    pub books: Vec<BookSummaryWithPresence>,
    pub sort_order: FolderViewSort,
    pub scroll_offset: usize,
    pub cursor_pos: usize,
    pub cursor_on: CursorTarget,
}

#[derive(Debug, Clone)]
pub enum CursorTarget {
    Folder(FolderId),
    Book(BookId),
}

pub struct BookSummaryWithPresence {
    pub summary: BookSummary,
    pub presence: FilePresence,
}

#[derive(Debug, Clone)]
pub enum FolderViewSort {
    FoldersFirst,       // Папки наверху, затем книги
    Mixed,              // Перемешаны по имени
    BooksFirst,         // Книги наверху
}

pub struct RenameState {
    pub target: RenameTarget,
    pub original_name: String,
    pub current_input: String,
    pub cursor_pos: usize,
}

pub enum RenameTarget {
    Folder(FolderId),
    Book(BookId),
}
```

---

## 11. Интеграция с AI

### 11.1 AI-операции специфичные для папок

```
@p  (в FOLDER mode) → Omniscope: предложить реструктуризацию текущей папки
@t  (в FOLDER mode) → Omniscope: предложить теги для всех книг в папке
@a  (в FOLDER mode) → Omniscope: аудит текущей папки (дубли, ghost, orphaned)

:ai restructure       Предложить новую организацию папок на основе контента
:ai create-folders    Создать рекомендованную структуру папок для темы
:ai name-folder       Предложить название для папки на основе книг внутри
```

### 11.2 AI и Ghost books

```
Omniscope знает о ghost books через Library Map.
В LibraryMap::BookSummaryCompact добавить поле:
  "ghost": true  — если file_presence == NeverHadFile
  "detached": true — если file_presence == Missing

Пример проактивного уведомления:
  "У тебя 8 ghost books по теме 'transformers'. Все они доступны на arXiv.
   Скачать PDF для всех? [Да] [Показать список] [Позже]"

AI-действие FetchAndAdd может создавать ghost books:
  → Добавить карточку с метаданными без скачивания файла
  → book.file_presence = NeverHadFile
  → Пользователь позже скачивает через gf или :oa download
```

### 11.3 Auto-organize через AI

```
:ai auto-organize        → AI предлагает переструктурировать все папки
                           Показывает preview перемещений, ждёт подтверждения

:ai auto-organize --apply → Применить сразу (осторожно!)

Пример сессии:
  User: сгруппируй все статьи по годам внутри ml-papers

  Omniscope: Анализирую ml-papers/ (38 книг)...
             Предлагаю создать:
             • ml-papers/2017-2020/ (11 статей)
             • ml-papers/2021-2022/ (15 статей)
             • ml-papers/2023-2024/ (12 статей)

             [Создать папки и переместить] [Показать детали] [Отмена]
```

---

## 12. Roadmap реализации

### Этап F-0 (3 дня): Типы данных и миграция

```
□ Добавить FolderType enum в omniscope-core
□ Добавить FilePresence enum
□ Обновить BookCard: добавить file_presence, folder_id, virtual_folder_ids
□ SQLite миграция: таблицы folders, book_virtual_folders
□ SQLite: ALTER TABLE books ADD COLUMN folder_id, file_presence
□ Индексы: idx_folders_parent, idx_books_folder
□ Юнит-тесты: CRUD для folders в БД
□ Тест: BookCard roundtrip с FilePresence::NeverHadFile

Проверка: cargo test --package omniscope-core все зелёные
```

### Этап F-1 (4 дня): FolderTree и синхронизация

```
□ FolderTree::build() — из SQLite, < 50ms для 1000 папок
□ FolderTree::apply_change() — инкрементальные обновления
□ FolderSync::full_scan() — diff диска и БД
□ FolderSync::apply_sync() — применить с выбранной стратегией
□ FolderOps::create_folder() — mkdir + БД + дерево
□ FolderOps::rename_folder() — mv + обновить дочерние пути
□ FolderOps::move_folder() — mv + обновить всё поддерево
□ FolderOps::delete_folder() — KeepFiles и WithFiles режимы
□ FolderOps::move_book_to_folder() — mv файла + обновить БД
□ CLI: omniscope folder create/rename/move/delete/tree/info --json
□ CLI: omniscope sync --dry-run --json

Проверка:
  omniscope folder create programming/rust/async --json (mkdir создан)
  mv ~/Books/programming/rust ~/Books/programming/rust-lang
  omniscope sync --dry-run → обнаружил переименование
```

### Этап F-2 (3 дня): Filesystem Watcher

```
□ LibraryWatcher::start() — notify интеграция
□ Обработка: NewBookFile, BookFileRemoved, DirectoryCreated, DirectoryRemoved
□ EventDebouncer — 2000ms задержка
□ Размерный фильтр (min_file_size_kb)
□ Интеграция в TUI event loop: handle_next_event()
□ Уведомление в статус-баре при новом файле
□ CLI: omniscope watch start/stop/status --json
□ Конфиг: [watcher] в library.toml

Проверка:
  Скопировать PDF в ~/Books/ → через 2с уведомление в TUI
  Удалить файл → книга становится detached
```

### Этап F-3 (5 дней): Левая панель Folder Tree

```
□ LeftPanelMode enum: LibraryView / FolderTree
□ FolderTreePanel::render() с иконками, отступами, счётчиками
□ Ghost count индикатор: "34+2○"
□ Раздел Virtual Folders
□ Tab: переключение LibraryView ↔ FolderTree
□ FOLDER mode state machine (отдельный от Normal)
□ FOLDER mode навигация: j/k/h/l, gg/G, za/zo/zc/zR/zM
□ FOLDER mode: a (create), r (rename), dd (delete), m/p (move)
□ Inline rename: поле ввода прямо в дереве, Esc/Enter
□ Visual mode: V + d/m операторы на папках
□ Статус-бар в FOLDER mode: "FOLDER  rust/   34 books"
□ / поиск по именам папок

Проверка: создать, переименовать, переместить, удалить папку через TUI
```

### Этап F-4 (4 дня): Folder View в центральной панели

```
□ CenterPanelMode enum: BookList / FolderView
□ gv: переключить BookList ↔ FolderView
□ FolderContents: подпапки + книги в виде единого списка
□ Хлебные крошки (breadcrumb) в заголовке центральной панели
□ Enter на папке → войти; - → выйти в родителя
□ Ghost books: dim стиль + индикатор "○ ghost"
□ DetachedBooks: orange ⚠ индикатор
□ Сортировка: FoldersFirst / Mixed / BooksFirst (T переключение)
□ Все vim операторы работают на смешанных папка/книга элементах
□ Preview в правой панели: папка → статистика, книга → карточка
□ FolderViewSort в конфиге

Проверка: навигировать по ~/Books через Folder View, открывать книги
```

### Этап F-5 (3 дня): Sync Panel и Ghost Books UX

```
□ SyncReport отображение в TUI (Sync Panel)
□ :sync / @sync открывает панель
□ Визуальная классификация: NEW / DETACHED / UNTRACKED DIRS
□ Опции для каждого элемента: [Import] [Ignore] [Relink] [Keep as ghost]
□ Кнопка [Apply all] с preview + подтверждением
□ Watcher уведомления: импортировать панель новых файлов
□ Ghost books: gf → "Find & Download" через Sci-Hub/Anna's
□ Ghost books: E → attach existing file picker
□ CLI: omniscope book list --ghost/--detached --json
□ CLI: omniscope book attach/locate/detach --json

Проверка: Sync Panel правильно показывает расхождения, позволяет импортировать
```

### Этап F-6 (2 дня): AI интеграция и полировка

```
□ AI: @p / :ai restructure для папок
□ AI-действие AutoOrganize: preview + confirm
□ AI: LibraryMap включает ghost_count по папкам
□ Конфиг watcher и folder_view в library.toml
□ :import new — панель импорта новых файлов
□ Бенчмарк: FolderTree::build() < 50ms для 1000 папок
□ E2E тест: init → добавить файлы → sync → правильная структура

Проверка: все сценарии из §13
```

---

## 13. E2E сценарии для тестирования

### Сценарий A: Импорт существующей коллекции

```bash
# Пользователь имеет ~/Books/ с существующей структурой папок
cd ~/Books
omniscope init

# Сканирование обнаруживает существующие директории и файлы
omniscope sync --dry-run
# → 23 директории, 147 файлов книг, 0 в БД

omniscope sync --strategy disk-wins
# → Создать 23 папки в БД + 147 карточек с minimal metadata

# TUI показывает правильное дерево
omniscope  # открыть TUI → gF → видим дерево ~/Books/ идентичное диску
```

### Сценарий B: Реструктуризация через FOLDER mode

```
1. gF → войти в FOLDER mode
2. Найти папку "misc/" (содержит 30 разнородных книг)
3. a → создать подпапку "misc/rust/" → Enter
4. a → создать "misc/ml/" → Enter
5. l → войти в misc/ (центральная панель показывает содержимое)
6. V → выбрать все rust книги
7. m → переместить в misc/rust/ (файлы физически перемещаются)
8. Esc → выйти из Visual
9. Повторить для ML книг
10. Итог: misc/rust/ и misc/ml/ на диске
```

### Сценарий C: Ghost books workflow

```
1. omniscope arxiv add 1706.03762  (без --download-pdf)
   → Создаётся GhostBook "Attention Is All You Need"
   → file_presence = NeverHadFile

2. В Folder View: видим книгу с 󰈖 dim иконкой "○ ghost"

3. Нажать gf → "Find PDF?"
   → Поиск через Unpaywall (open access) → найден
   → Скачать? [Да] [Позже]
   → После: file_presence = Present { path: "ml-papers/1706.03762.pdf" }

4. Книга теперь отображается как обычная PhysicalBook
```

### Сценарий D: Watcher auto-detection

```
1. Watcher запущен (автоматически при старте TUI)

2. Пользователь скачивает "новая-книга.pdf" в ~/Books/programming/rust/

3. Через 2 секунды в TUI:
   Статус-бар: "[+1 new file]" (мигает)

4. Нажать Space → открыть Import Panel
   Видим: "async-rust-oreilly.pdf" (2.4 MB)
   Обнаружены метаданные: "Async Rust" · O'Reilly · 2024

5. [Import] → создаётся карточка, книга появляется в дереве

6. Или: watcher.auto_import = true → карточка создаётся автоматически,
   уведомление в статус-баре: "Auto-imported: Async Rust"
```

### Сценарий E: Sync после ручного перемещения файлов

```
# Пользователь ВРУЧНУЮ (без TUI) переместил папку:
mv ~/Books/ml-papers/transformers ~/Books/ml-papers/transformers-2024

# При следующем открытии TUI (или :sync):
Sync Panel показывает:
  ⚠ DETACHED (12): 12 книг ссылаются на несуществующие пути
  ⊕ NEW (0)
  󰉋 UNTRACKED DIRS (1): ml-papers/transformers-2024/

Действие: [Auto-relink by hash]
  → Omniscope вычислил hash файлов перед и после → нашёл совпадения
  → Обновил пути для 11 книг автоматически
  → 1 книга не найдена (удалена) → остаётся Detached

Результат: папка переименована в БД, все пути актуальны
```

---

*Диск — источник правды. TUI — зеркало диска. Все операции — двусторонние.*
*Ghost books — полноценные участники: метаданные без файла лучше, чем ничего.*
*Никаких сюрпризов: preview + undo для каждой деструктивной операции.*
