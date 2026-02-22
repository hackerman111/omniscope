# 🗄️ OMNISCOPE — Архитектура хранилища: Полная спецификация

> Этот документ описывает переработанную модель хранения данных Omniscope.
> Ключевое изменение: библиотека живёт **рядом с книгами**, а не в `~/.local`.
> Философия — как `.git` для репозитория: зашёл в папку с книгами, библиотека уже там.

---

## 0. Философия хранения

### Старая модель (отвергнута)
```
~/.local/share/omniscope/     ← метаданные
~/Books/                      ← файлы книг
```
Проблема: два места. Перенёс папку с книгами — потерял связь с метаданными.
Сделал бэкап книг — метаданные не попали. Неинтуитивно.

### Новая модель
```
~/Books/
├── .libr/                    ← библиотека живёт здесь, рядом с книгами
│   ├── library.toml          ← манифест библиотеки
│   ├── cards/                ← JSON-карточки
│   ├── index/                ← tantivy + SQLite
│   └── ...
├── programming/
│   ├── rust/
│   │   └── trpl.pdf
│   └── algorithms/
└── ml-papers/
    └── attention.pdf
```

Преимущества:
- Переместил папку — библиотека переехала вместе с ней
- `cp -r ~/Books /backup/` — бэкап включает все метаданные
- Несколько независимых библиотек на одном компьютере
- Понятно без документации: `.libr` = "это папка с книгами под управлением omniscope"

| Принцип | Описание |
|---|---|
| **"Библиотека = папка"** | `.libr/` рядом с книгами, как `.git` рядом с кодом |
| **"Переноси папку — переносишь всё"** | Нет скрытых зависимостей в `~/.local` |
| **"Бэкап = одна папка"** | `tar -czf backup.tar.gz ~/Books/` — полный бэкап |
| **"Несколько библиотек"** | Каждая корневая папка независима |

---

## 1. Структура `.libr/` — детально

```
{books_root}/
└── .libr/
    │
    ├── library.toml              Манифест: имя, версия схемы, настройки
    ├── lock                      Lock-файл (предотвращает одновременный доступ)
    │
    ├── cards/                    JSON-карточки книг
    │   ├── {uuid}.json
    │   ├── {uuid}.json
    │   └── ...
    │
    ├── db/
    │   ├── omniscope.db          SQLite: books, tags, libraries, frecency, action_log
    │   └── tantivy/              Full-text индекс
    │       ├── meta.json
    │       └── ...
    │
    ├── vectors/
    │   └── embeddings.usearch    HNSW векторный индекс (если RAG включён)
    │
    ├── cache/
    │   ├── covers/               Обложки книг {uuid}.jpg
    │   ├── crossref/             Кэш CrossRef API
    │   ├── s2/                   Кэш Semantic Scholar
    │   ├── annas/                Кэш Anna's Archive
    │   └── library_map.json      Кэш Library Map для AI
    │
    ├── undo/
    │   └── action_log.jsonl      Лог AI-действий с snapshot_before
    │
    └── backups/
        ├── 2025-02-19_full.omnibak    Полный бэкап (бинарный архив)
        └── 2025-02-19_full.omnibak.sha256
```

### 1.1 `library.toml` — манифест библиотеки

```toml
[library]
name = "My Library"
id = "ulid-01HXYZ..."          # Уникальный ID библиотеки (для синхронизации)
version = 1                     # Версия схемы .libr/ (для миграций)
created_at = "2025-02-19T10:00:00Z"
omniscope_version = "0.3.0"    # Версия создавшая библиотеку

[library.roots]
# Дополнительные корневые папки этой же библиотеки (необязательно)
# Позволяет иметь книги в нескольких местах под одной библиотекой
extra = [
    "/media/external-drive/Books",
    "/home/user/Papers",
]

[settings]
# Переопределения глобального конфига для этой библиотеки
default_viewer_pdf = "zathura"
language = "en"
auto_index = true
```

---

## 2. Обнаружение библиотеки (Discovery)

Omniscope ищет `.libr/` по принципу git: от текущей директории вверх.

```rust
pub fn discover_library(start: &Path) -> Option<LibraryRoot> {
    // 1. Проверить переменную окружения
    if let Ok(path) = std::env::var("OMNISCOPE_LIBRARY") {
        return Some(LibraryRoot::new(PathBuf::from(path)));
    }

    // 2. Подняться от текущей директории вверх
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".libr");
        if candidate.is_dir() && candidate.join("library.toml").exists() {
            return Some(LibraryRoot::new(current));
        }
        if !current.pop() {
            break;
        }
    }

    // 3. Проверить список известных библиотек из глобального конфига
    for path in global_config().known_libraries() {
        if path.join(".libr").exists() {
            return Some(LibraryRoot::new(path));
        }
    }

    None
}
```

**Глобальный конфиг** (`~/.config/omniscope/config.toml`) хранит только:
- Список известных библиотек
- Глобальные настройки (тема, API-ключи, viewer defaults)
- Ничего специфичного для конкретной библиотеки

```toml
# ~/.config/omniscope/config.toml

[global]
theme = "catppuccin-mocha"
default_editor = "$EDITOR"

[ai]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"

# Реестр известных библиотек
[[libraries]]
name = "My Library"
path = "/home/user/Books"
id = "ulid-01HXYZ..."

[[libraries]]
name = "Work Papers"
path = "/home/user/Papers"
id = "ulid-02HABC..."
```

---

## 3. Инициализация библиотеки

### `omniscope init` — создать новую библиотеку

```bash
# В текущей директории
cd ~/Books
omniscope init

# Или с указанием пути
omniscope init ~/Books
omniscope init ~/Books --name "My Library"

# Создать папку и инициализировать
omniscope init ~/NewBooks --create-dir
```

```rust
pub async fn init_library(root: &Path, opts: InitOptions) -> Result<LibraryRoot> {
    // 1. Создать директорию если нужно
    if opts.create_dir {
        tokio::fs::create_dir_all(root).await?;
    }

    // Проверить что директория существует
    ensure!(root.exists(), "Directory does not exist: {}", root.display());

    // Проверить что нет уже существующей библиотеки
    let libr = root.join(".libr");
    ensure!(!libr.exists(), ".libr already exists in {}", root.display());

    // 2. Создать структуру .libr/
    for dir in &["cards", "db", "db/tantivy", "vectors", "cache",
                 "cache/covers", "cache/crossref", "cache/s2", "cache/annas",
                 "undo", "backups"] {
        tokio::fs::create_dir_all(libr.join(dir)).await?;
    }

    // 3. Создать library.toml
    let manifest = LibraryManifest {
        name: opts.name.unwrap_or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("My Library")
                .to_string()
        }),
        id: Ulid::new().to_string(),
        version: SCHEMA_VERSION,
        created_at: Utc::now(),
        omniscope_version: env!("CARGO_PKG_VERSION").to_string(),
        ..Default::default()
    };
    write_toml(&libr.join("library.toml"), &manifest).await?;

    // 4. Создать SQLite схему
    let db = SqlitePool::connect(&libr.join("db/omniscope.db").to_str().unwrap()).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    // 5. Зарегистрировать в глобальном конфиге
    global_config_mut().add_library(manifest.id.clone(), root.to_path_buf())?;

    // 6. Опциональный импорт существующих файлов
    if opts.scan_existing {
        scan_and_import_files(root, &db).await?;
    }

    println!("✓ Initialized omniscope library in {}", root.display());
    println!("  Run 'omniscope scan' to import existing files.");

    Ok(LibraryRoot::new(root.to_path_buf()))
}
```

**TUI flow для `omniscope init`:**
```
┌─────────────────────────────────────────────────────────────┐
│  Initialize Omniscope Library                               │
├─────────────────────────────────────────────────────────────┤
│  Path:   /home/user/Books                                   │
│  Name:   [My Library                                      ] │
│                                                             │
│  ┌ Options ──────────────────────────────────────────────┐  │
│  │ [x] Scan existing files and create cards              │  │
│  │ [x] Auto-detect metadata from files                   │  │
│  │ [ ] Create standard folder structure                  │  │
│  │ [ ] Add to global registry                            │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│              [Cancel]    [Initialize →]                     │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Управление папками библиотеки

Omniscope умеет создавать и управлять папочной структурой на диске.
Папки в `.libr` и реальные папки на диске синхронизированы.

### 4.1 Создание папок

```bash
# Создать папку (и папку на диске)
omniscope folder create programming/rust
omniscope folder create programming/rust --description "Rust books"

# Создать вложенную иерархию
omniscope folder create ml-papers/2024/transformers

# Создать стандартную структуру (шаблон)
omniscope folder scaffold --template research
omniscope folder scaffold --template personal
```

```rust
pub async fn create_folder(
    library: &LibraryRoot,
    path: &str,            // "programming/rust/async"
    opts: FolderCreateOpts,
) -> Result<FolderInfo> {

    // 1. Создать реальную директорию на диске
    let disk_path = library.root().join(path);
    tokio::fs::create_dir_all(&disk_path).await
        .context(format!("Failed to create directory: {}", disk_path.display()))?;

    // 2. Добавить запись в SQLite
    let folder = FolderInfo {
        id: Ulid::new().to_string(),
        path: path.to_string(),
        disk_path: disk_path.clone(),
        description: opts.description,
        created_at: Utc::now(),
        icon: opts.icon,
        color: opts.color,
    };
    db.insert_folder(&folder).await?;

    // 3. Если создаём промежуточные папки — зарегистрировать и их
    for ancestor in path_ancestors(path) {
        if !db.folder_exists(ancestor).await? {
            db.insert_folder(&FolderInfo::from_path(ancestor)).await?;
        }
    }

    Ok(folder)
}
```

### 4.2 Шаблоны структуры

```rust
pub enum FolderTemplate {
    /// Для исследователей/студентов
    Research,
    /// Для личной библиотеки
    Personal,
    /// Для технической литературы
    Technical,
    /// Пользовательский (из TOML файла)
    Custom(PathBuf),
}

// Research шаблон создаёт:
// papers/
//   ├── reading/
//   ├── read/
//   ├── to-read/
//   ├── by-topic/
//   └── by-year/
// books/
//   ├── textbooks/
//   └── reference/
// notes/

// Technical шаблон:
// programming/
//   ├── rust/
//   ├── python/
//   ├── systems/
//   └── algorithms/
// reference/
// courses/
```

```bash
# Показать что будет создано перед применением
omniscope folder scaffold --template research --dry-run

# ── Preview ─────────────────────────────────────────────────
# Will create:
#   ~/Books/papers/
#   ~/Books/papers/reading/
#   ~/Books/papers/read/
#   ~/Books/papers/to-read/
#   ~/Books/papers/by-topic/
#   ~/Books/papers/by-year/
#   ~/Books/books/
#   ~/Books/books/textbooks/
#   ~/Books/books/reference/
# Apply? [y/N]
```

### 4.3 Синхронизация папок: диск ↔ библиотека

```rust
/// Обнаружить расхождения между диском и метаданными
pub async fn sync_folders(library: &LibraryRoot) -> SyncReport {
    let disk_folders = scan_directories(library.root()).await;
    let db_folders = db.list_folders().await;

    let mut report = SyncReport::new();

    // Папки на диске, которых нет в БД
    for folder in &disk_folders {
        if !db_folders.contains(folder) {
            report.new_on_disk.push(folder.clone());
        }
    }

    // Папки в БД, которых нет на диске (удалены)
    for folder in &db_folders {
        if !disk_folders.contains(folder) {
            report.missing_on_disk.push(folder.clone());
        }
    }

    // Файлы не прикреплённые ни к какой карточке
    let untracked = scan_untracked_files(library.root()).await;
    report.untracked_files = untracked;

    report
}
```

```
┌─────────────────────────────────────────────────────────────────────┐
│  SYNC: Disk ↔ Library                                               │
├─────────────────────────────────────────────────────────────────────┤
│  ✓  12 folders in sync                                              │
│  ⊕   3 new folders found on disk (not in library)                  │
│       programming/zig/         [Add to library]                     │
│       papers/2025/             [Add to library]                     │
│       archive/old/             [Add to library] [Ignore]            │
│  ⊘   1 folder in library missing on disk                            │
│       programming/haskell/     [Remove from library] [Recreate dir] │
│  ？  7 untracked files (no card)                                     │
│       book_without_card.pdf    [Create card] [Ignore]               │
│       ...                                                           │
├─────────────────────────────────────────────────────────────────────┤
│  [Sync All] [Review Each] [Auto-fix Safe Issues]                    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. Система бэкапа

### 5.1 Форматы бэкапа

```rust
pub enum BackupFormat {
    /// Полный бинарный архив (.omnibak)
    /// Содержит: все карточки + SQLite + индексы + настройки
    /// НЕ содержит: файлы книг (только метаданные)
    Full,

    /// Полный архив включая файлы книг (.omnibak.full)
    /// Может быть очень большим
    WithFiles,

    /// Только карточки в читаемом JSON (.omniexport)
    /// Максимальная переносимость, минимальный размер
    CardsOnly,

    /// Только карточки + структура папок
    /// Достаточно для воссоздания архитектуры библиотеки
    Skeleton,
}

pub struct BackupManifest {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub omniscope_version: String,
    pub library_name: String,
    pub library_id: String,
    pub format: BackupFormat,
    pub stats: BackupStats,
    pub checksum: String,       // SHA-256 всего архива
    pub encrypted: bool,
}

pub struct BackupStats {
    pub total_books: u32,
    pub with_files: u32,
    pub libraries_count: u32,
    pub folders_count: u32,
    pub tags_count: u32,
    pub total_size_bytes: u64,
}
```

### 5.2 Создание бэкапа

```bash
# Бэкап метаданных (быстрый, маленький)
omniscope backup create
omniscope backup create --output ~/backups/library.omnibak
omniscope backup create --format cards-only

# Бэкап с файлами (полный, большой)
omniscope backup create --with-files --output ~/backups/full.omnibak

# Автоматический бэкап по расписанию
omniscope backup schedule --interval weekly --keep 4 --output ~/backups/

# Зашифрованный бэкап
omniscope backup create --encrypt --password-env BACKUP_PASSWORD

# Проверить целостность бэкапа
omniscope backup verify ~/backups/library.omnibak
```

```rust
pub async fn create_backup(library: &LibraryRoot, opts: BackupOptions) -> Result<PathBuf> {
    let output = opts.output.unwrap_or_else(|| {
        library.libr_dir()
            .join("backups")
            .join(format!("{}_backup.omnibak", Utc::now().format("%Y-%m-%d")))
    });

    let mut archive = BackupArchive::new(&output, opts.format)?;

    // 1. Записать манифест
    let stats = gather_stats(library).await?;
    archive.write_manifest(&BackupManifest::new(library, opts.format, stats))?;

    // 2. Все JSON-карточки
    for card_path in list_cards(library).await? {
        archive.add_file("cards/", &card_path)?;
    }

    // 3. SQLite дамп (портабельный, не бинарный файл БД)
    let sql_dump = dump_sqlite_portable(library).await?;
    archive.write_bytes("db/dump.sql", &sql_dump)?;

    // 4. Структура папок
    let folder_tree = export_folder_tree(library).await?;
    archive.write_json("structure/folders.json", &folder_tree)?;

    // 5. Теги и библиотеки
    archive.write_json("structure/tags.json", &export_tags(library).await?)?;
    archive.write_json("structure/libraries.json", &export_libraries(library).await?)?;

    // 6. Настройки (library.toml без секретов)
    archive.add_file("config/", &library.libr_dir().join("library.toml"))?;

    // 7. Файлы книг (опционально)
    if opts.include_files {
        for book in books_with_files(library).await? {
            archive.add_file("files/", &book.file.path)?;
        }
    }

    // 8. Зашифровать если нужно
    if let Some(password) = opts.password {
        archive.encrypt(&password)?;
    }

    // 9. Записать SHA-256
    let checksum = archive.finalize()?;
    tokio::fs::write(output.with_extension("omnibak.sha256"),
                     checksum.to_hex()).await?;

    Ok(output)
}
```

### 5.3 Воссоздание библиотеки из бэкапа

Ключевая операция: `omniscope restore`. Умеет работать в нескольких режимах.

```bash
# Полное восстановление в новую директорию
omniscope restore ~/backups/library.omnibak --into ~/Books-restored

# Восстановление поверх существующей (merge)
omniscope restore ~/backups/library.omnibak --into ~/Books --mode merge

# Восстановить только структуру (папки + теги + библиотеки, без карточек)
omniscope restore ~/backups/library.omnibak --skeleton-only

# Восстановить конкретную библиотеку/тег
omniscope restore ~/backups/library.omnibak --filter "library:programming"
omniscope restore ~/backups/library.omnibak --filter "tag:rust"

# Preview что будет восстановлено
omniscope restore ~/backups/library.omnibak --dry-run
```

```rust
pub async fn restore_library(
    backup_path: &Path,
    opts: RestoreOptions,
) -> Result<RestoreReport> {

    // 1. Открыть и верифицировать бэкап
    let archive = BackupArchive::open(backup_path)?;
    archive.verify_checksum()?;
    let manifest = archive.read_manifest()?;

    println!("📦 Backup: {} ({} books, created {})",
             manifest.library_name,
             manifest.stats.total_books,
             manifest.created_at.format("%Y-%m-%d"));

    // 2. Подготовить целевую директорию
    let target = &opts.into;
    match opts.mode {
        RestoreMode::Fresh => {
            // Инициализировать чистую библиотеку
            ensure!(!target.join(".libr").exists(),
                    "Target already has a .libr directory. Use --mode merge or choose different path.");
            init_library(target, InitOptions::minimal()).await?;
        }
        RestoreMode::Merge => {
            // Убедиться что цель — библиотека
            ensure!(target.join(".libr").exists(),
                    "Target is not an omniscope library. Run 'omniscope init' first.");
        }
        RestoreMode::SkeletonOnly => {
            if !target.join(".libr").exists() {
                init_library(target, InitOptions::minimal()).await?;
            }
        }
    }

    let library = LibraryRoot::open(target)?;
    let mut report = RestoreReport::new();

    // 3. Восстановить структуру папок (ВСЕГДА первым)
    let folder_tree: Vec<FolderInfo> = archive.read_json("structure/folders.json")?;
    for folder in &folder_tree {
        let disk_path = target.join(&folder.path);
        tokio::fs::create_dir_all(&disk_path).await?;
        db.upsert_folder(folder).await?;
        report.folders_restored += 1;
    }

    // 4. Восстановить теги
    let tags: Vec<Tag> = archive.read_json("structure/tags.json")?;
    for tag in &tags {
        db.upsert_tag(tag).await?;
        report.tags_restored += 1;
    }

    // 5. Восстановить библиотеки (группы верхнего уровня)
    let libraries: Vec<Library> = archive.read_json("structure/libraries.json")?;
    for lib in &libraries {
        db.upsert_library(lib).await?;
        report.libraries_restored += 1;
    }

    if opts.mode == RestoreMode::SkeletonOnly {
        return Ok(report);
    }

    // 6. Восстановить карточки книг
    for card_entry in archive.list_cards()? {
        let mut card: BookCard = archive.read_json(&card_entry)?;

        // Обработать конфликты при merge
        if opts.mode == RestoreMode::Merge {
            if let Some(existing) = db.find_by_id(&card.id).await? {
                match opts.conflict {
                    ConflictStrategy::SkipExisting => {
                        report.skipped += 1;
                        continue;
                    }
                    ConflictStrategy::OverwriteWithBackup => {
                        // Перезаписать карточку из бэкапа
                    }
                    ConflictStrategy::KeepNewer => {
                        if existing.updated_at > card.updated_at {
                            report.skipped += 1;
                            continue;
                        }
                    }
                    ConflictStrategy::AskUser => {
                        // В TUI режиме — показать диалог
                        // В CLI — пропустить и добавить в report.conflicts
                        report.conflicts.push(card.id.clone());
                        continue;
                    }
                }
            }
        }

        // Переписать путь к файлу если файлы не перенесены
        if let Some(file) = &mut card.file {
            let new_path = target.join(
                file.path.strip_prefix(&manifest.original_root)
                    .unwrap_or(&file.path)
            );
            if new_path.exists() {
                file.path = new_path;
                report.files_relinked += 1;
            } else {
                file.path = new_path;  // Сохранить путь, файл ещё не скопирован
                report.files_missing += 1;
            }
        }

        db.upsert_card(&card).await?;
        write_card_json(&library, &card).await?;
        report.cards_restored += 1;
    }

    // 7. Восстановить файлы книг (если были в бэкапе)
    if archive.has_files() {
        for file_entry in archive.list_files()? {
            let dest = target.join(&file_entry.relative_path);
            tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
            archive.extract_file(&file_entry, &dest)?;
            report.files_restored += 1;
        }
    }

    // 8. Перестроить индексы
    rebuild_tantivy_index(&library).await?;
    // Эмбеддинги не восстанавливаются (пересчитываются по запросу)

    println!("{}", report.summary());
    Ok(report)
}
```

### 5.4 TUI для восстановления

```
┌─────────────────────────────────────────────────────────────────────┐
│  🔄 RESTORE LIBRARY                                                  │
├─────────────────────────────────────────────────────────────────────┤
│  Backup:   ~/backups/2025-02-19_backup.omnibak         [Browse...]  │
│  ✓ Valid backup — SHA-256 verified                                  │
│                                                                     │
│  Content:                                                           │
│    147 books   |   8 libraries   |   34 tags   |   23 folders       │
│    Created: 2025-02-19  |  Format: Full (no files)                  │
│                                                                     │
│  Restore into:  [/home/user/Books-restored            ] [Browse...] │
│                                                                     │
│  Mode:                                                              │
│    (●) Fresh — create new library from backup                       │
│    ( ) Merge — merge with existing library                          │
│    ( ) Skeleton only — folders + tags + structure (no book cards)   │
│                                                                     │
│  On conflict (for merge):                                           │
│    (●) Keep newer   ( ) Skip existing   ( ) Overwrite              │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ [x] Restore folder structure         (23 folders)           │   │
│  │ [x] Restore tags and libraries       (34 tags, 8 libs)      │   │
│  │ [x] Restore book cards               (147 cards)            │   │
│  │ [ ] Copy files from backup           (no files in backup)   │   │
│  │ [x] Rebuild search index after restore                      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  [Dry Run →]                        [Restore →]                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.5 Dry-run вывод

```
$ omniscope restore ~/backups/library.omnibak --into ~/Books-new --dry-run

📦 Backup Analysis
  Library: "My Library" (147 books, created 2025-02-19)
  Format: Full (no files included)
  Checksum: ✓ Valid

📁 Will create directory: /home/user/Books-new
📁 Will initialize .libr/ in /home/user/Books-new

📂 Folder structure (23 folders):
  + programming/
  + programming/rust/
  + programming/rust/official/
  + programming/algorithms/
  + ml-papers/
  + ml-papers/transformers/
  + ml-papers/rl/
  ... and 16 more

🏷  Tags (34):
  + rust, python, algorithms, systems, async, transformers ... (+28)

📚 Books (147):
  + The Rust Programming Language (Klabnik 2023) [pdf — file not in backup]
  + Attention Is All You Need (Vaswani 2017)     [pdf — file not in backup]
  ... and 145 more

⚠  Files not included in backup:
   147 books have file references but files were not backed up.
   After restore, cards will point to: /home/user/Books-new/{original_relative_path}
   Run 'omniscope sync' after copying your files to relink them.

📊 Summary (dry run):
  Folders:   23 created
  Tags:      34 restored
  Libraries:  8 restored
  Cards:    147 restored
  Files:      0 (not in backup)
  Index:    rebuilt after restore

Run without --dry-run to apply.
```

---

## 6. Сканирование существующих файлов

При первом `omniscope init` или вручную через `omniscope scan`.

```bash
# Сканировать корневую папку библиотеки
omniscope scan

# Сканировать конкретную папку
omniscope scan ~/Books/programming/

# С автоматическим созданием карточек
omniscope scan --auto-create-cards

# С извлечением метаданных из файлов (медленнее)
omniscope scan --extract-metadata

# С обогащением через внешние API (ещё медленнее)
omniscope scan --enrich
```

```
┌─────────────────────────────────────────────────────────────────────┐
│  📂 SCANNING ~/Books/...                              [████░░░░ 47%]│
├─────────────────────────────────────────────────────────────────────┤
│  Found:    234 files                                                │
│  Known:     89 (already have cards)                                 │
│  New:      145 (no card yet)                                        │
│  Ignored:    0                                                      │
│                                                                     │
│  Currently: programming/algorithms/taocp-vol1.pdf                  │
│             → Extracting PDF metadata...                            │
│             → Title: "The Art of Computer Programming, Vol. 1"     │
│             → Author: Knuth, Donald E.                              │
│             → ISBN found: 978-0-201-89683-1                         │
├─────────────────────────────────────────────────────────────────────┤
│  [Pause]  [Cancel]  [Skip metadata extraction]                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 7. Несколько библиотек

Omniscope поддерживает несколько независимых библиотек.

```bash
# Список всех известных библиотек
omniscope libraries
# →  My Library        ~/Books          (147 books)  [current]
# →  Work Papers       ~/Papers         (89 books)
# →  Archive           /mnt/nas/Books   (312 books)

# Переключиться на другую библиотеку
omniscope switch "Work Papers"
omniscope switch ~/Papers

# Открыть TUI с конкретной библиотекой
omniscope --library ~/Papers

# Забыть библиотеку из реестра (не удаляет файлы)
omniscope libraries forget ~/old/path
```

**Переменная окружения для скриптов:**
```bash
OMNISCOPE_LIBRARY=~/Papers omniscope book list --json
```

**TUI переключатель библиотек (`gb` — go buffers):**
```
┌────────────────────────────────────────┐
│  LIBRARIES                             │
├────────────────────────────────────────┤
│  ▶ My Library          ~/Books   147  │
│    Work Papers         ~/Papers   89  │
│    Archive             /mnt/nas  312  │
│                                        │
│  [Enter] switch  [i] init new  [f] forget│
└────────────────────────────────────────┘
```

---

## 8. Перенос библиотеки

```bash
# Переместить библиотеку в новое место
# (просто mv — .libr/ переедет вместе с книгами)
mv ~/Books ~/NewLocation/Books

# Обновить путь в глобальном реестре
omniscope libraries update-path ~/Books --new-path ~/NewLocation/Books

# Или — omniscope сам обнаружит после перемещения:
cd ~/NewLocation/Books
omniscope    # Найдёт .libr/ здесь, обновит реестр автоматически
```

```rust
/// При открытии — проверить что путь в реестре совпадает с текущим
fn reconcile_library_path(discovered: &Path, registered: &Option<PathBuf>) {
    if let Some(reg) = registered {
        if reg != discovered {
            // Путь изменился — обновить реестр молча
            global_config_mut().update_library_path(reg, discovered);
            log::info!("Library moved from {} to {}", reg.display(), discovered.display());
        }
    } else {
        // Новая библиотека — зарегистрировать
        global_config_mut().add_library_from_path(discovered);
    }
}
```

---

## 9. Влияние на другие документы проекта

### Изменения в `AI_PLAN.md`

**§2.2 Структура на диске** — заменить на:
```
{books_root}/
└── .libr/
    ├── library.toml
    ├── cards/{uuid}.json
    ├── db/omniscope.db
    ├── db/tantivy/
    ├── cache/covers/
    ├── undo/action_log.jsonl
    └── backups/

~/.config/omniscope/config.toml    (только глобальные настройки + реестр)
```

**§17 SLA** — добавить:
- `omniscope init` < 500ms для пустой директории
- `omniscope scan` (без метаданных) < 10ms/файл
- `omniscope restore` (только карточки, без файлов) < 1s/100 карточек

**§18 CLI** — добавить команды:
```bash
omniscope init [path]                     # Инициализировать библиотеку
omniscope scan [--extract-metadata]       # Сканировать файлы
omniscope sync                            # Синхронизировать диск ↔ БД
omniscope libraries                       # Список библиотек
omniscope libraries forget [path]         # Удалить из реестра
omniscope switch [name|path]              # Переключить библиотеку
omniscope backup create [--with-files]    # Создать бэкап
omniscope backup restore [path]           # Восстановить из бэкапа
omniscope backup verify [path]            # Проверить целостность
omniscope backup schedule [--interval]    # Настроить автобэкап
```

### Изменения в `MASTER_PLAN.md`

**Фаза 0** — добавить в §0.4:
```
□ LibraryRoot::discover() — поиск .libr/ от текущей директории вверх
□ omniscope init — полная реализация с wizard
□ omniscope scan — сканирование существующих файлов
□ Глобальный реестр библиотек в ~/.config/omniscope/config.toml
□ Reconcile при открытии (автообновление пути после mv)
□ Тест: init → mv → открыть — путь обновлён автоматически
```

**Фаза 1** — добавить в §1.5:
```
□ create_folder() создаёт реальную папку на диске
□ sync_folders(): обнаружение расхождений диск ↔ БД
□ FolderTemplate::scaffold() — создание типовых структур
□ TUI sync-панель: ⊕ новые / ⊘ пропавшие / ？ неотслеживаемые файлы
```

**Фаза 5** — переименовать в "Импорт/Экспорт + Бэкап", добавить:
```
□ BackupArchive: create, open, verify
□ omniscope backup create / restore / verify / schedule
□ RestoreMode: Fresh / Merge / SkeletonOnly
□ ConflictStrategy: SkipExisting / OverwriteWithBackup / KeepNewer
□ Dry-run для restore с полным отчётом
□ Тест: backup → удалить .libr/ → restore → идентичная библиотека
```

---

## 10. Формат `.omnibak` — бинарная структура

```
[4 bytes]  Magic: 0x4F4D4E49 ("OMNI")
[4 bytes]  Format version: 1
[4 bytes]  Flags: 0x01=encrypted, 0x02=compressed, 0x04=with_files
[8 bytes]  Timestamp (Unix, little-endian)
[4 bytes]  Manifest length
[N bytes]  Manifest (JSON, gzip compressed)
[4 bytes]  TOC length
[N bytes]  Table of Contents (JSON список файлов с offsets)
[N bytes]  Payload (все файлы, каждый gzip compressed)
[32 bytes] SHA-256 всего предыдущего содержимого
```

Читаемый альтернативный формат `.omniexport` — это просто `.tar.gz` с известной структурой. Можно открыть любым архиватором и прочитать карточки вручную.

---

*`.libr/` — это не скрытая директория с магическими файлами. Это твоя библиотека, живущая рядом с твоими книгами, открытая и переносимая.*
