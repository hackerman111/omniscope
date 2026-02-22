# 🗂️ Пошаговый план реализации системы папок Omniscope

> **Файлы затронуты:** `omniscope-core`, `omniscope-tui`, `omniscope-cli`
> **Спек:** `OMNISCOPE_FOLDER_SYSTEM.md` — читать указанный раздел ПЕРЕД каждым шагом
> **Правило агента:** после каждого шага — `cargo test`, `cargo clippy --deny warnings`. Красные тесты = стоп.
> **Коммит:** каждый шаг — один атомарный коммит с осмысленным сообщением

---

## Принципы, которые нельзя нарушать

```
1. "Папка = директория на диске" — каждая Physical-папка имеет реальный путь в ФС
2. "Перемести в TUI — переместишь файл" — все мутирующие операции двусторонние
3. "Диск — источник правды" — при конфликте данные диска побеждают
4. "Никаких сюрпризов" — деструктивные операции только с preview + undo
5. Атомарность: если mkdir прошёл, а запись в БД упала — откатить mkdir (и наоборот)
6. Операции над папками всегда обновляют дерево FolderTree инкрементально, без полного rebuild
7. Ghost-книги (NeverHadFile) — полноправные объекты, не second-class citizens
```

---

## Шаг 0. Ошибки и вспомогательные типы

**Референс:** `FOLDER_SYSTEM.md §0` (таксономия), `§1.1` (типы структур)

**Файлы:** `omniscope-core/src/models/folder.rs`, `omniscope-core/src/errors.rs`

**Что сделать:**

Добавить `FolderError` в систему ошибок крейта:

- `FolderNotFound(FolderId)`
- `DiskError { path: PathBuf, source: std::io::Error }` — ошибка операции на диске
- `NameConflict { path: PathBuf }` — директория с таким именем уже существует
- `InvalidName(String)` — нарушение правил именования
- `CircularMove { folder_id: FolderId }` — попытка переместить папку внутрь себя
- `RootCannotBeDeleted` — попытка удалить LibraryRoot
- `NotPhysical(FolderId)` — операция требует Physical папку, получена Virtual
- `BookNotFound(BookId)`
- `FileHashMismatch { expected: String, actual: String }`

Объявить базовые типы-алиасы (ещё без реализации):

```
type FolderId = String;     // UUID
type RelativePath = String; // "programming/rust" от корня библиотеки
type AbsolutePath = PathBuf;
```

Написать тест: `FolderError::InvalidName` форматируется нормально через `Display`.

**Проверка:** `cargo build --package omniscope-core` без ошибок.

---

## Шаг 1. Базовые типы данных — Folder, FolderType, FilePresence, BookCard

**Референс:** `FOLDER_SYSTEM.md §1.1`

**Файл:** `omniscope-core/src/models/folder.rs`

**Что сделать:**

Объявить все структуры данных из `§1.1` **без логики** — только типы и трейты:

`FolderType` enum:
- `Physical` — реальная директория на диске
- `Virtual` — только в метаданных (нет `disk_path`)
- `LibraryRoot` — корень библиотеки

`FilePresence` enum:
- `Present { path: AbsolutePath, size_bytes: u64, hash: Option<String> }`
- `NeverHadFile` — ghost book
- `Missing { last_known_path: AbsolutePath, last_seen: DateTime<Utc> }` — detached book

`Folder` struct: `id: FolderId`, `name: String`, `folder_type: FolderType`, `parent_id: Option<FolderId>`, `library_id: LibraryId`, `disk_path: Option<RelativePath>`, `icon: Option<String>`, `color: Option<String>`, `sort_index: i32`, `created_at`, `updated_at`. Все поля с `#[serde(default, skip_serializing_if = "Option::is_none")]`.

Расширить `BookCard` двумя новыми полями:
- `file_presence: FilePresence` (default: `NeverHadFile`)
- `folder_id: Option<FolderId>` — принадлежность к физической папке
- `virtual_folder_ids: Vec<FolderId>` — может быть в нескольких virtual folders

Написать тесты: serde roundtrip для `FilePresence::Present`, `FilePresence::NeverHadFile`, `FilePresence::Missing`. Проверить что `FolderType::Virtual` сериализуется и десериализуется без потерь.

**Проверка:** `cargo test -p omniscope-core models` — всё зелёное.

---

## Шаг 2. SQLite-схема и миграция

**Референс:** `FOLDER_SYSTEM.md §1.2`

**Файл:** `omniscope-core/src/db/migrations/`

**Что сделать:**

Создать миграцию (следующий номер после существующих):

```sql
-- Таблица папок
CREATE TABLE folders (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    folder_type TEXT NOT NULL CHECK(folder_type IN ('physical', 'virtual', 'library_root')),
    parent_id   TEXT REFERENCES folders(id) ON DELETE CASCADE,
    library_id  TEXT NOT NULL REFERENCES libraries(id),
    disk_path   TEXT,
    icon        TEXT,
    color       TEXT,
    sort_index  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- M:N связь книга ↔ виртуальная папка
CREATE TABLE book_virtual_folders (
    book_id    TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    folder_id  TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    added_at   TEXT NOT NULL,
    PRIMARY KEY (book_id, folder_id)
);

-- Расширение таблицы books
ALTER TABLE books ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
ALTER TABLE books ADD COLUMN file_presence TEXT NOT NULL DEFAULT 'never_had_file';
ALTER TABLE books ADD COLUMN file_last_seen TEXT;
ALTER TABLE books ADD COLUMN file_hash TEXT;

-- Индексы
CREATE INDEX idx_folders_parent   ON folders(parent_id);
CREATE INDEX idx_folders_library  ON folders(library_id);
CREATE INDEX idx_folders_disk_path ON folders(disk_path) WHERE disk_path IS NOT NULL;
CREATE INDEX idx_books_folder     ON books(folder_id);
CREATE INDEX idx_books_presence   ON books(file_presence);
```

Реализовать CRUD-методы в `Database`:
- `create_folder(folder: &Folder) -> Result<()>`
- `get_folder(id: &FolderId) -> Result<Folder>`
- `get_folder_by_path(disk_path: &RelativePath, library_id: &LibraryId) -> Result<Option<Folder>>`
- `list_folders(parent_id: Option<&FolderId>) -> Result<Vec<Folder>>`
- `update_folder(folder: &Folder) -> Result<()>`
- `rename_folder(id: &FolderId, new_name: &str) -> Result<()>`
- `delete_folder(id: &FolderId) -> Result<()>` — CASCADE удалит дочерние
- `update_folder_path_recursive(folder_id: &FolderId, old_prefix: &str, new_prefix: &str) -> Result<u32>` — SQL `UPDATE folders SET disk_path = replace(disk_path, old, new) WHERE disk_path LIKE old_prefix || '%'`, вернуть кол-во обновлённых строк
- `move_folder(id: &FolderId, new_parent_id: Option<&FolderId>) -> Result<()>`
- `count_books_in_subtree(folder_id: &FolderId) -> Result<u32>` — рекурсивный CTE
- `detach_books_from_folder(folder_id: &FolderId) -> Result<()>` — `UPDATE books SET folder_id = NULL, file_presence = 'missing' WHERE folder_id = ?`
- `update_book_folder(book_id: &BookId, folder_id: Option<&FolderId>) -> Result<()>`
- `update_book_file_path(book_id: &BookId, new_path: &AbsolutePath) -> Result<()>`
- `update_book_paths_in_folder(folder_id: &FolderId, old_prefix: &str, new_prefix: &str) -> Result<u32>`
- `get_books_by_presence(presence_type: &str) -> Result<Vec<BookCard>>` — для ghost/detached списков
- `add_book_to_virtual_folder(book_id: &BookId, folder_id: &FolderId) -> Result<()>`
- `remove_book_from_virtual_folder(book_id: &BookId, folder_id: &FolderId) -> Result<()>`

Написать тесты на in-memory SQLite:
- создать папку → получить по ID → данные совпадают
- создать дерево папок (root → parent → child) → `list_folders(Some(parent_id))` возвращает только child
- `update_folder_path_recursive` обновляет все дочерние пути
- `count_books_in_subtree` возвращает сумму по всему поддереву
- `delete_folder` → дочерние папки каскадно удаляются

**Проверка:** `cargo test -p omniscope-core db` — все зелёные.

---

## Шаг 3. FolderTree — дерево в памяти

**Референс:** `FOLDER_SYSTEM.md §1.3`

**Файл:** `omniscope-core/src/models/folder_tree.rs`

**Что сделать:**

`FolderNode` struct: `folder: Folder`, `children: Vec<FolderNode>`, `book_count: u32` (включая поддерево), `book_count_direct: u32`, `ghost_count: u32`, `is_expanded: bool`.

`FolderTree` struct: `root: FolderNode`, `index: HashMap<FolderId, *mut FolderNode>` (unsafe указатели для O(1) доступа), `path_index: HashMap<RelativePath, FolderId>`, `library_id: LibraryId`.

⚠ Unsafe-указатели требуют `unsafe impl Send + Sync` и чёткого документирования инвариантов: указатели валидны пока жив `FolderTree`, не утекать наружу.

`FolderTreeChange` enum: `FolderCreated(Folder)`, `FolderRenamed { id, new_name }`, `FolderMoved { id, new_parent_id }`, `FolderDeleted(FolderId)`, `BookCountChanged { folder_id, delta: i32 }`.

Реализовать методы:

`FolderTree::build(db: &Database, library_id: &LibraryId) -> Result<Self>` — загрузить все папки одним запросом (`SELECT * FROM folders WHERE library_id = ?`), построить дерево за один проход через HashMap. **Бенчмарк-цель: < 50ms для 1000 папок.** Счётчики книг заполнять снизу вверх (post-order traversal).

`FolderTree::find_by_id(&self, id: &FolderId) -> Option<&FolderNode>` — O(1) через `index`.

`FolderTree::find_by_path(&self, path: &RelativePath) -> Option<&FolderNode>` — O(1) через `path_index`.

`FolderTree::breadcrumb(&self, folder_id: &FolderId) -> Vec<&Folder>` — путь от корня до узла.

`FolderTree::children(&self, folder_id: &FolderId) -> &[FolderNode]` — прямые дочерние узлы.

`FolderTree::apply_change(&mut self, change: FolderTreeChange)` — инкрементально обновить дерево, индексы и счётчики без полного rebuild:
- `FolderCreated` → вставить новый узел в нужную позицию, обновить `index` и `path_index`, инкрементировать счётчики родителей
- `FolderRenamed` → обновить `folder.name`, перепостроить `path_index` для поддерева
- `FolderMoved` → извлечь узел из старого родителя, вставить в новый, перепостроить `path_index` для поддерева, скорректировать счётчики обоих родителей
- `FolderDeleted` → удалить из `index`, `path_index` и из `children` родителя, декрементировать счётчики
- `BookCountChanged` → инкрементировать `book_count` вверх по дереву до корня

Написать тесты:
- `build` из 1000 папок завершается < 50ms (используйте `std::time::Instant`)
- `find_by_id` возвращает корректный узел
- `apply_change(FolderCreated)` → узел появляется в дереве и в index
- `apply_change(FolderDeleted)` → узел исчезает из дерева и из обоих индексов
- `apply_change(FolderMoved)` → счётчики обновляются у обоих родителей
- `breadcrumb` возвращает путь в правильном порядке
- `apply_change(BookCountChanged { delta: 1 })` → инкрементирует счётчики вверх до корня

**Проверка:** `cargo test -p omniscope-core folder_tree` — все зелёные, бенчмарк проходит.

---

## Шаг 4. validate_folder_name

**Референс:** `FOLDER_SYSTEM.md §2.3` — функция `validate_folder_name`

**Файл:** `omniscope-core/src/models/folder.rs`

**Что сделать:**

Функция `validate_folder_name(name: &str) -> Result<(), FolderError>` — проверить все условия:
- не пустая строка
- длина ≤ 255 символов
- не содержит `/` (разделитель пути)
- не содержит нулевой байт `\0`
- не является `.` или `..`
- не начинается с пробела и не заканчивается пробелом (trim guard)
- не содержит управляющих символов (ASCII < 32)

На Windows дополнительно: не содержит `<>:"/\|?*`, не является зарезервированным именем (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`). Использовать `#[cfg(target_os = "windows")]` для платформозависимых проверок.

Написать тесты (все до реализации):
- `"rust"` → Ok
- `"rust-lang"` → Ok
- `"async"` → Ok (ключевое слово языка, но валидное имя папки)
- `""` → Err(InvalidName)
- `"/"` → Err(InvalidName)
- `"a/b"` → Err(InvalidName)
- `"."` → Err(InvalidName)
- `".."` → Err(InvalidName)
- `" leading"` → Err(InvalidName)
- `"trailing "` → Err(InvalidName)
- строка из 256 символов → Err(InvalidName)

**Проверка:** все тесты зелёные.

---

## Шаг 5. FolderOps — атомарные операции

**Референс:** `FOLDER_SYSTEM.md §2.3`

**Файл:** `omniscope-core/src/sync/folder_ops.rs`

Это самый критичный модуль. Каждая операция должна быть атомарна: либо всё (диск + БД + дерево + action_log), либо ничего (откат).

`FolderOps` struct: `library_root: PathBuf`, `db: Arc<Database>`, `tree: Arc<RwLock<FolderTree>>`, `action_log: Arc<ActionLog>`.

### 5а. create_folder

`async fn create_folder(parent_id, name, folder_type) -> Result<Folder>`

Логика:
1. `validate_folder_name(name)?`
2. Вычислить `new_rel_path = parent_path + "/" + name`
3. Если `Physical`: `tokio::fs::create_dir_all(&abs_path).await?` — если ошибка, вернуть `FolderError::DiskError` (БД не трогаем)
4. `db.create_folder(&folder).await?` — если ошибка, откатить mkdir: `let _ = tokio::fs::remove_dir(&abs_path).await`
5. `tree.write().apply_change(FolderTreeChange::FolderCreated(...))`
6. `action_log.append(...)` с `snapshot_after`
7. Поддержать вложенный путь: `"theory/advanced"` → создать промежуточную папку `theory/` если не существует, затем `theory/advanced/`

Тест: создать папку → проверить что директория существует на диске + запись есть в БД. Тест: имя с `/` → ошибка ещё до mkdir.

### 5б. rename_folder

`async fn rename_folder(folder_id, new_name) -> Result<()>`

Логика:
1. `validate_folder_name(new_name)?`
2. Получить `folder` из БД — сохранить `snapshot_before`
3. Проверить что `new_abs_path` не существует → `FolderError::NameConflict`
4. `tokio::fs::rename(&old_abs, &new_abs).await?` — если ошибка, вернуть (БД не тронута)
5. `db.update_folder_path_recursive(folder_id, &old_rel_str, &new_rel_str).await?` — обновить пути папки и всех дочерних
6. `db.rename_folder(folder_id, new_name).await?`
7. `db.update_book_paths_in_folder(folder_id, &old_rel_str, &new_rel_str).await?` — обновить пути файлов книг
8. `tree.write().apply_change(FolderTreeChange::FolderRenamed {...})`
9. `action_log.append(...)` с `snapshot_before`

Тест: переименовать папку → проверить что директория на диске переименована, путь в БД обновлён, пути у дочерних папок тоже обновлены.

### 5в. move_folder

`async fn move_folder(folder_id, new_parent_id: Option<FolderId>) -> Result<()>`

Логика:
1. Проверить отсутствие циклического перемещения: `new_parent_id` не должен быть потомком `folder_id` → `FolderError::CircularMove`
2. Проверить что `new_abs_path` не существует → `FolderError::NameConflict`
3. `tokio::fs::rename(&old_abs, &new_abs).await?`
4. `db.update_folder_path_recursive(...)` — обновить всё поддерево
5. `db.update_book_paths_in_folder(...)` — обновить пути файлов
6. `db.move_folder(folder_id, new_parent_id).await?`
7. `tree.write().apply_change(FolderTreeChange::FolderMoved {...})`
8. `action_log.append(...)` с `snapshot_before`

Тест: переместить папку с тремя дочерними → все дочерние имеют обновлённый `disk_path`. Тест: попытка переместить папку внутрь её же потомка → `CircularMove`.

### 5г. delete_folder

`async fn delete_folder(folder_id, mode: FolderDeleteMode) -> Result<DeleteFolderReport>`

`FolderDeleteMode` enum: `KeepFiles` (только убрать из библиотеки, файлы на диске не трогать), `WithFiles` (удалить директорию и всё содержимое).

Логика:
1. Проверить что папка не `LibraryRoot` → `FolderError::RootCannotBeDeleted`
2. `affected_books = db.count_books_in_subtree(folder_id).await?`
3. Собрать `DeleteFolderReport` для preview (не применять!)
4. Если `WithFiles`: `tokio::fs::remove_dir_all(&abs_path).await?`
5. Если `KeepFiles`: `db.detach_books_from_folder(folder_id).await?` (книги → `Missing`)
6. `db.delete_folder(folder_id).await?` — CASCADE удалит дочерние папки из БД
7. `tree.write().apply_change(FolderTreeChange::FolderDeleted(...))`
8. `action_log.append(...)` с `snapshot_before`

Тест: удалить папку с тремя книгами в режиме `KeepFiles` → книги в БД с `file_presence = Missing`, директория на диске существует. Тест: `WithFiles` → директория удалена с диска.

### 5д. move_book_to_folder

`async fn move_book_to_folder(book_id, target_folder_id) -> Result<()>`

Логика:
1. Получить `book` и `target_folder`
2. Если `book.file_presence == NeverHadFile` (ghost book) → только `db.update_book_folder(...)`, файл не трогать
3. Если `Present { path }` → вычислить `new_file_path = target_dir / filename`, вызвать `resolve_name_conflict(&new_file_path)` если файл с таким именем уже существует (добавить суффикс `_2`, `_3` и т.д.)
4. `tokio::fs::rename(&old_path, &final_path).await?`
5. `db.update_book_file_path(book_id, &final_path).await?`
6. `db.update_book_folder(book_id, Some(target_folder_id)).await?`
7. `tree.write().apply_change(BookCountChanged { folder_id: old_folder_id, delta: -1 })` + `BookCountChanged { folder_id: target_folder_id, delta: 1 }`

`resolve_name_conflict(path: &Path) -> PathBuf` — если путь не существует, вернуть as-is. Иначе добавлять суффикс `_2`, `_3` пока не найдём свободное имя.

Тест: переместить книгу → файл физически переместился, `book.folder_id` обновился. Тест: ghost book → только метаданные обновляются.

**Проверка:** `cargo test -p omniscope-core folder_ops` — все зелёные.

---

## Шаг 6. FolderSync — сканирование и диффинг

**Референс:** `FOLDER_SYSTEM.md §2.2`

**Файл:** `omniscope-core/src/sync/folder_sync.rs`

`FolderSync` struct: `library_root: PathBuf`, `db: Arc<Database>`, `tree: Arc<RwLock<FolderTree>>`.

`SyncReport` struct: `untracked_dirs: Vec<PathBuf>`, `orphaned_folders: Vec<FolderId>`, `untracked_files: Vec<PathBuf>`, `missing_files: Vec<BookId>`, `moved_files: Vec<(BookId, PathBuf)>`.

`SyncStrategy` enum: `DiskWins`, `Interactive`.

`SyncApplyResult` struct: `folders_created: u32`, `folders_orphaned: u32`, `books_imported: u32`, `books_relinked: u32`, `pending_review: Option<SyncReport>`.

### 6а. scan_disk

`async fn scan_disk(&self) -> Result<DiskState>`

Обойти дерево директорий рекурсивно через `tokio::fs::read_dir`. Игнорировать:
- директории начинающиеся с `.` (в т.ч. `.libr/`)
- паттерны из конфига (`ignore_patterns`: `.DS_Store`, `Thumbs.db`, `__pycache__`, `*.tmp`)

`is_book_file(path: &Path) -> bool` — проверить расширение: `pdf`, `epub`, `djvu`, `fb2`, `mobi`, `azw3`, `cbz`, `cbr`.

Возвращает `DiskState { dirs: Vec<PathBuf>, files: Vec<PathBuf> }`.

### 6б. load_db_state

`async fn load_db_state(&self) -> Result<DbState>`

Загрузить из БД: все `Physical` папки библиотеки (с их `disk_path`), все книги (с их `file_path` из `file_presence`).

`DbState { folders: HashMap<RelativePath, FolderId>, book_paths: HashMap<AbsolutePath, BookId> }`.

### 6в. diff

`fn diff(&self, disk: DiskState, db: DbState) -> SyncReport`

Сравнить:
- `disk.dirs` ∖ `db.folders` → `untracked_dirs`
- `db.folders` ∖ `disk.dirs` → `orphaned_folders`
- `disk.files` ∖ `db.book_paths` → `untracked_files`
- `db.book_paths` ∖ `disk.files` → `missing_files`
- для каждого `missing_file`: вычислить hash из `db`, поискать файл с таким же hash среди `untracked_files` → `moved_files`

Хэширование для moved detection: `sha256` первых 64KB файла (быстро). Использовать крейт `sha2`. Только если `verify_file_hashes = true` в конфиге, иначе `moved_files` пустой.

### 6г. apply_sync

`async fn apply_sync(&self, report: &SyncReport, strategy: SyncStrategy) -> Result<SyncApplyResult>`

Логика из `§2.2`:
- `DiskWins`: для каждого `untracked_dir` → `import_directory`, для `orphaned_folders` → `mark_folder_orphaned`, для `untracked_files` → `auto_import_file`
- `Interactive`: вернуть `SyncApplyResult { pending_review: Some(report.clone()), .. }` — TUI покажет панель
- Всегда: для `missing_files` → `update_file_presence(Missing)`, для `moved_files` → `update_book_path`

`import_directory(&self, dir: &Path) -> Result<FolderId>` — вычислить relative path, найти parent в `path_index`, создать `Folder` в БД, обновить дерево.

`auto_import_file(&self, path: &Path, strategy: SyncStrategy) -> Result<BookId>` — создать `BookCard` с `file_presence = Present`, минимальными метаданными (только имя файла). AI-обогащение — позже, по отдельной команде.

Тест: создать временную директорию с файловой структурой → `full_scan` → репорт содержит корректные `untracked_dirs` и `untracked_files`. Тест: переместить файл вручную → `diff` обнаруживает в `moved_files`.

**Проверка:** `cargo test -p omniscope-core folder_sync` — зелёные.

---

## Шаг 7. LibraryWatcher — filesystem watcher

**Референс:** `FOLDER_SYSTEM.md §3.1`

**Файл:** `omniscope-core/src/sync/watcher.rs`

**Зависимость:** добавить `notify = "6"` в `Cargo.toml` — использует inotify (Linux), kqueue (macOS), ReadDirectoryChangesW (Windows).

`WatcherConfig` struct (из `§3.1`): `auto_import: bool` (default: `false`), `debounce_ms: u64` (default: `2000`), `min_file_size_bytes: u64` (default: `10 * 1024`), `watch_extensions: Vec<String>`.

`WatcherEvent` enum (из `§3.1`): `NewBookFile { path }`, `BookFileRemoved { path }`, `BookFileRenamed { from, to }`, `DirectoryCreated { path }`, `DirectoryRemoved { path }`, `DirectoryRenamed { from, to }`.

`WatcherAction` enum: `AutoImport { path }`, `NotifyNewFile { path }`, `MarkFileDetached { path }`, `SyncNewDirectory { path }`, `MarkFolderOrphaned { path }`, `UpdateBookPath { old_path, new_path }`.

`LibraryWatcher` struct: `_watcher: RecommendedWatcher`, `event_rx: mpsc::Receiver<WatcherEvent>`, `config: WatcherConfig`, `folder_sync: Arc<FolderSync>`, `debouncer: EventDebouncer`.

### 7а. EventDebouncer

`EventDebouncer { window_ms: u64, pending: HashMap<PathBuf, (EventKind, Instant)> }`.

Логика: получить raw notify event → добавить в `pending` с timestamp. Перед возвратом следующего события — проверить что с момента последнего события прошло `window_ms`. Для rename detection: если получен `Remove(path1)` и затем `Create(path2)` в пределах `window_ms` → выдать `Renamed { from: path1, to: path2 }` вместо двух отдельных событий.

Минимальный размер файла: перед `NewBookFile` проверить `metadata.len() >= min_file_size_bytes`. Если файл ещё копируется (меньше минимума) — подождать ещё `debounce_ms` и перепроверить.

### 7б. LibraryWatcher::start

`fn start(library_root, folder_sync, config) -> Result<Self>`

Создать два канала: `raw_tx/raw_rx` (сырые notify события, буфер 256) и `event_tx/event_rx` (обработанные события, буфер 64).

Создать `RecommendedWatcher` с callback → `raw_tx.blocking_send(event)`. Запустить `watch(&library_root, RecursiveMode::Recursive)`.

Запустить `tokio::spawn` с `process_events(raw_rx, event_tx, library_root, config)`:
- Фильтровать события из `.libr/` директории
- Пропускать директории начинающиеся с `.`
- Применять debouncer
- Конвертировать `notify::EventKind` → `WatcherEvent`

### 7в. handle_next_event

`async fn handle_next_event(&mut self) -> Option<WatcherAction>`

Метод вызывается из TUI event loop в каждой итерации. Не блокирует — возвращает `None` если нет событий.

Конвертирует `WatcherEvent` → `WatcherAction`:
- `NewBookFile { path }` → если `auto_import` → `AutoImport { path }`, иначе `NotifyNewFile { path }`
- `BookFileRemoved { path }` → `MarkFileDetached { path }`
- `DirectoryCreated { path }` → `SyncNewDirectory { path }`
- `DirectoryRemoved { path }` → `MarkFolderOrphaned { path }`
- `BookFileRenamed { from, to }` → `UpdateBookPath { old_path: from, new_path: to }`

Тест: создать файл во временной директории → через `debounce_ms + 100ms` → `handle_next_event` возвращает `NotifyNewFile`. Тест: файл меньше `min_file_size_bytes` → события нет до тех пор, пока файл не достигнет нужного размера.

**Проверка:** `cargo test -p omniscope-core watcher` — зелёные.

---

## Шаг 8. Конфигурация папочной системы

**Референс:** `FOLDER_SYSTEM.md §9`

**Файл:** `omniscope-core/src/config/folder_config.rs`

`FolderConfig` struct с секциями из `§9`:

```
[folders]
creation_mode: CreationMode  // Manual | AutoFromDisk
auto_sync_dirs: bool         // default: true
ignore_patterns: Vec<String> // [".DS_Store", "Thumbs.db", ...]

[watcher]
enabled: bool                // default: true
debounce_ms: u64             // default: 2000
auto_import: bool            // default: false
min_file_size_kb: u64        // default: 10
extensions: Vec<String>

[sync]
default_strategy: SyncStrategy    // Interactive | DiskWins | LibraryWins
verify_file_hashes: bool          // default: false
auto_sync_interval_minutes: u32   // default: 0 (manual only)

[folder_view]
show_ghost_books: bool            // default: true
ghost_style: GhostStyle           // Dimmed | Strikethrough | Labeled
entry_order: EntryOrder           // FoldersFirst | Mixed | BooksFirst

[left_panel]
default_mode: LeftPanelMode       // LibraryView | FolderTree
show_ghost_count: bool            // default: true
show_virtual_folders: bool        // default: true
```

`FolderConfig::load(library_root: &Path) -> Result<Self>` — читать `.libr/library.toml`, fallback на `Default::default()`. `FolderConfig::save(&self, library_root: &Path) -> Result<()>`.

`fn matches_ignore_pattern(path: &Path, patterns: &[String]) -> bool` — glob-матчинг через крейт `glob`. Написать тесты: `".DS_Store"` матчится, `"normal.pdf"` не матчится, `"*.tmp"` матчится `"file.tmp"`.

---

## Шаг 9. CLI команды

**Референс:** `FOLDER_SYSTEM.md §8`

**Файл:** `omniscope-cli/src/commands/folder.rs`

Реализовать все команды из `§8` через `clap`. Каждая команда поддерживает `--json` для machine-readable вывода.

**folder subcommands:**

`folder create <path> [--virtual] [--json]` — вызвать `FolderOps::create_folder`. Принимать как `"programming/rust/async"` (создаёт вложенную структуру), так и просто `"async"` (в текущем контексте библиотеки). Вывод: `{"status":"ok","folder":{"id":"...","name":"async","disk_path":"..."}}`.

`folder rename <path|id> <new-name> [--by-id] [--json]` — вызвать `FolderOps::rename_folder`.

`folder move <path|id> <new-parent-path|id> [--by-id] [--json]` — вызвать `FolderOps::move_folder`.

`folder delete <path|id> [--with-files] [--confirm] [--json]` — если `--with-files` без `--confirm` → вывести preview и запросить подтверждение в stdin. Вызвать `FolderOps::delete_folder`.

`folder tree [path] [--depth <n>] [--json]` — красивое ASCII-дерево или JSON.

`folder info <path|id> [--json]` — `{"folder":{...}, "book_count":34, "ghost_count":2, "disk_size_mb":145}`.

**sync subcommands:**

`sync [--dry-run] [--strategy <disk-wins|interactive>] [--check-files-only] [--json]` — вызвать `FolderSync::full_scan()` + если не `--dry-run` → `apply_sync`.

`scan <path> [--auto-import] [--json]` — сканировать конкретную директорию.

`watch start|stop|status [--json]` — запустить/остановить/проверить watcher (через PID-файл или сигналы).

**book subcommands (расширение существующих):**

`book list --ghost [--json]` и `book list --detached [--json]` — вызвать `db.get_books_by_presence`.

`book move <id...> --folder <path|id> [--json]` — вызвать `FolderOps::move_book_to_folder` для каждого ID.

`book virtual-add <book-id> --virtual-folder <name|id> [--json]`.

`book virtual-remove <book-id> --virtual-folder <name|id> [--json]`.

`book attach <book-id> --file <path> [--json]` — прикрепить файл к ghost book: проверить что файл существует, скопировать в нужную директорию, обновить `file_presence = Present`.

`book detach <book-id> [--json]` — конвертировать `Present` или `Missing` → `NeverHadFile` (убрать путь, оставить метаданные).

`book locate <book-id> --sources <...> [--json]` — заглушка, полная реализация в `omniscope-science`.

Написать интеграционный тест (с временной директорией): `folder create` → проверить директорию на диске. `folder rename` → проверить новое имя. `folder delete --with-files --confirm` → директория удалена.

**Проверка:** `cargo test -p omniscope-cli folder` — все зелёные.

---

## Шаг 10. TUI State — FolderState

**Референс:** `FOLDER_SYSTEM.md §10`

**Файл:** `omniscope-tui/src/state/folder_state.rs`

Объявить все структуры состояния из `§10` **без логики** — только типы:

`FolderState` struct: `left_mode: LeftPanelMode`, `folder_tree: Arc<RwLock<FolderTree>>`, `tree_cursor: FolderTreeCursor`, `tree_expand_state: HashMap<FolderId, bool>`, `center_mode: CenterPanelMode`, `current_folder: Option<FolderId>`, `breadcrumb: Vec<Folder>`, `folder_contents: FolderContents`, `pending_watcher_events: VecDeque<WatcherAction>`, `rename_state: Option<RenameState>`, `sync_report: Option<SyncReport>`, `sync_panel_visible: bool`.

`LeftPanelMode` enum: `LibraryView`, `FolderTree`.

`CenterPanelMode` enum: `BookList`, `FolderView`.

`FolderContents` struct: `subfolders: Vec<FolderNode>`, `books: Vec<BookSummaryWithPresence>`, `sort_order: FolderViewSort`, `scroll_offset: usize`, `cursor_pos: usize`, `cursor_on: CursorTarget`.

`CursorTarget` enum: `Folder(FolderId)`, `Book(BookId)`.

`FolderViewSort` enum: `FoldersFirst`, `Mixed`, `BooksFirst`.

`RenameState` struct: `target: RenameTarget`, `original_name: String`, `current_input: String`, `cursor_pos: usize`.

`RenameTarget` enum: `Folder(FolderId)`, `Book(BookId)`.

`FolderTreeCursor` struct: `selected_id: Option<FolderId>`, `visual_line: usize`, `scroll_offset: usize`.

`BookSummaryWithPresence` struct: `summary: BookSummary`, `presence: FilePresence`.

Добавить `FolderState` как поле в глобальный `AppState`.

Реализовать `FolderState::new(tree: Arc<RwLock<FolderTree>>, config: &FolderConfig) -> Self` — инициализировать с `left_mode = config.left_panel.default_mode`.

**Проверка:** `cargo build -p omniscope-tui` без ошибок.

---

## Шаг 11. Левая панель — FolderTreePanel, рендер

**Референс:** `FOLDER_SYSTEM.md §4.1, §4.2, §4.3`

**Файл:** `omniscope-tui/src/panels/left/folder_tree.rs`

`FolderTreePanel` struct: `tree: Arc<RwLock<FolderTree>>`, `cursor: FolderTreeCursor`, `mode: LeftPanelMode`, `expand_state: HashMap<FolderId, bool>`, `virtual_section_visible: bool`.

### 11а. flatten_tree

`fn flatten_tree(&self) -> Vec<FlatFolderNode>`

`FlatFolderNode` struct: `folder: Folder`, `depth: usize`, `is_expanded: bool`, `has_children: bool`, `book_count: u32`, `ghost_count: u32`.

Рекурсивный обход: если `is_expanded` → рекурсивно добавлять дочерние. Виртуальные папки — в отдельной секции после разделителя `─── VIRTUAL ───`.

### 11б. render

`fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_active: bool)`

Рендер заголовка: `" LIBRARIES "` в `LibraryView` режиме, `" FOLDERS "` в `FolderTree` режиме. Граница: `theme.frost_ice` если панель активна, `theme.border` если нет.

`render_folder_row(frame, node, x, y, width, theme)` — одна строка:
- Отступ: `"  ".repeat(depth)`
- Символ fold: `"▾ "` (развёрнута) / `"▸ "` (свёрнута) / `"  "` (нет дочерних)
- Иконка из `§4.2`: `"󰝰 "` (Physical expanded), `"󰉋 "` (Physical collapsed), `"⊕ "` (Virtual)
- Имя папки: `theme.fg_bright + BOLD` если выбрана, иначе `theme.fg`
- Счётчик: если `ghost_count > 0` → `"34+2○"` (nord3 для числа, nord12 для `+2○`), иначе просто `"34"`
- Фон выбранной строки: `theme.bg_secondary`

Виртуализация: отображать только строки от `scroll_offset` до `scroll_offset + area.height`.

Разделитель секций Virtual: `"─── VIRTUAL ─────────"` серым цветом (nord2).

Кнопка `[+] New folder` в самом низу панели.

Написать снапшот-тест (используя `insta` или ручной assert): `flatten_tree` для дерева из 5 папок возвращает корректный порядок с правильными depth.

### 11в. Tab переключение режима

Метод `FolderTreePanel::toggle_mode(&mut self)` — переключает `LibraryView ↔ FolderTree`, сбрасывает cursor на первый элемент.

**Проверка:** TUI запускается, левая панель отрисовывается в обоих режимах.

---

## Шаг 12. FOLDER mode — машина состояний

**Референс:** `FOLDER_SYSTEM.md §5.1, §5.2`

**Файл:** `omniscope-tui/src/input/folder_mode.rs`

`FolderModeState` enum: `Normal` (навигация), `Creating { input: String, cursor: usize, parent_id: Option<FolderId> }`, `Renaming(RenameState)`, `Searching { query: String, cursor: usize, results: Vec<FolderId> }`, `Visual { start_id: FolderId, selected: Vec<FolderId> }`, `RegisterMark { waiting_for: char }` (для `m{a-z}`).

`FolderModeRegisters` struct: `HashMap<char, FolderId>` — хранить папки в регистрах a-z.

Функция `handle_folder_mode_key(key: KeyEvent, state: &mut AppState, ops: &FolderOps) -> Result<Option<TuiCommand>>` — главный обработчик клавиш FOLDER mode.

Реализовать все привязки из `§5.2`:

**Навигация:**
- `j` / `k` — переместить cursor на следующую/предыдущую видимую строку в flat списке
- `gg` / `G` — первая/последняя папка
- `h` — если папка развёрнута → свернуть (`expand_state[id] = false`); если свёрнута → переместить cursor к родителю
- `l` / `Enter` — если папка свёрнута → развернуть; если развёрнута → открыть (переключить center на FolderView с этой папкой)
- `za` — toggle expand/collapse
- `zo` / `zc` — открыть/закрыть
- `zR` — развернуть все (`expand_state` = все `true`)
- `zM` — свернуть все (`expand_state` = все `false`)
- `{count}j` / `{count}k` — прыжок на N строк (накапливать цифры в `count_buffer`)
- `Ctrl+f` / `Ctrl+b` — прокрутка страницами
- `/` → перейти в `Searching` состояние
- `n` / `N` — следующий/предыдущий результат поиска

**Создание:**
- `a` / `gcf` → перейти в `Creating { parent_id: current_id }`
- `A` → `Creating { parent_id: current_id, folder_type: Virtual }`
- `o` → `Creating { parent_id: parent_of_current, position: After }`
- `O` → `Creating { parent_id: parent_of_current, position: Before }`

**Изменение:**
- `r` → `Renaming(RenameState { target: Folder(current_id), original_name, current_input: original_name })`
- `R` → то же, но сначала показать preview зависимостей (сколько книг/дочерних затронуто)
- `I` → только для Virtual: диалог выбора иконки
- `c` → только для Virtual: диалог выбора цвета

**Удаление:**
- `dd` → вызвать `FolderOps::delete_folder(current_id, KeepFiles)` (после preview-подтверждения)
- `dD` → `delete_folder(current_id, WithFiles)` (двойное подтверждение)
- `d_` → убрать из библиотеки, не трогая диск

**Перемещение:**
- `m{a-z}` → перейти в `RegisterMark`, ждать следующего символа → `registers.insert(char, current_id)`
- `p` → вызвать `FolderOps::move_folder(registers[default], Some(current_id))`
- `J` / `K` → изменить `sort_index` в БД, пересортировать в дереве
- `>` / `<` → углубить/поднять уровень (сделать дочерним для предыдущего соседа / поднять к родителю)

**Visual mode:**
- `V` → перейти в `Visual { start_id: current_id, selected: vec![current_id] }`
- В Visual `j`/`k` расширяют выделение, `d` → пакетное удаление, `m` → пакетное перемещение

**Информация:**
- `gi` → показать диалог с информацией о папке (путь, кол-во книг, размер)
- `gd` → открыть в файловом менеджере: `open .` (macOS), `xdg-open .` (Linux)
- `yy` → скопировать relative path в clipboard
- `yp` → скопировать absolute path в clipboard
- `?` → показать help overlay
- `Tab` → `toggle_mode()` (LibraryView ↔ FolderTree)
- `Esc` → вернуться в Normal mode, фокус на центральную панель

**Проверка:** ручное тестирование в TUI — создать, переименовать, переместить, удалить папку через клавиши.

---

## Шаг 13. Inline Create и Rename

**Референс:** `FOLDER_SYSTEM.md §5.3, §5.4`

**Файл:** `omniscope-tui/src/panels/left/folder_tree.rs` (расширение render)

### 13а. Inline Create

В `render_folder_row` проверять: если `mode == Creating` и `parent_id == current_folder.id` → после последнего ребёнка добавить пустое поле ввода.

Поле ввода рендерится как: `"  " + indent + "└ [" + input_text + " " * (width - len) + "]"` с курсором (мигающий `█`).

Обработка клавиш в `Creating` состоянии:
- Обычные символы → `input.push(char)`, `cursor += 1`
- `Backspace` → `input.pop()`, `cursor -= 1` если > 0
- `Ctrl+u` → очистить `input`, `cursor = 0`
- `Enter` → если `input` содержит `/` → создать вложенную структуру рекурсивно; иначе `FolderOps::create_folder(parent_id, &input, Physical).await` → показать статус "Created: {path}"; перейти в `Normal`
- `Esc` → выйти из `Creating` без создания

### 13б. Inline Rename

В `render_folder_row`: если `mode == Renaming` и `target == Folder(this_id)` → имя папки заменить полем ввода с текущим текстом, весь текст выделен.

Обработка клавиш в `Renaming` состоянии:
- Обычные символы → вставить в `current_input` в позицию `cursor`, `cursor += 1`
- `Ctrl+a` → выделить всё (cursor = 0, selection = all)
- `Tab` → автодополнение по другим папкам на том же уровне
- `Enter` → если `current_input != original_name` → `FolderOps::rename_folder(target_id, &current_input).await`; показать статус `"Renamed: '{original}' → '{new}'"` в статус-баре; перейти в `Normal`
- `Esc` → выйти из `Renaming` без изменений

Тест: начать rename → ввести новое имя → Enter → курсор остаётся на переименованной папке.

---

## Шаг 14. Поиск по дереву папок

**Референс:** `FOLDER_SYSTEM.md §5.2` — клавиша `/`

**Файл:** `omniscope-tui/src/panels/left/folder_search.rs`

В `Searching { query, cursor, results }` состоянии:

Рендер строки поиска внизу левой панели: `"/ {query}_"`.

Логика поиска: при каждом изменении `query` → `results = flatten_tree().filter(|n| n.folder.name.to_lowercase().contains(&query.to_lowercase()))`. Простое substring matching, без fuzzy.

При наличии результатов — подсвечивать совпадающие строки в дереве (`theme.search_highlight`).

Клавиши:
- `n` / `N` — следующий/предыдущий результат (по индексу в `results`)
- `Esc` / `Enter` — выйти из поиска, оставить cursor на текущем результате

---

## Шаг 15. Центральная панель — Folder View Mode

**Референс:** `FOLDER_SYSTEM.md §6.1, §6.2, §6.3`

**Файл:** `omniscope-tui/src/panels/center/folder_view.rs`

`FolderViewPanel` struct: `contents: FolderContents`, `breadcrumb: Vec<Folder>`, `theme: Theme`.

### 15а. load_folder_contents

`async fn load_folder_contents(folder_id: &FolderId, db: &Database, config: &FolderConfig) -> Result<FolderContents>`

Загрузить из БД:
1. Дочерние папки: `db.list_folders(Some(folder_id))`
2. Книги папки: `db.get_books_in_folder(folder_id)` → каждая с `BookSummaryWithPresence`

Сортировка по `config.folder_view.entry_order`:
- `FoldersFirst` — сначала все папки (по имени), затем все книги (по title)
- `Mixed` — всё вперемешку по имени
- `BooksFirst` — сначала книги, затем папки

### 15б. render

Макет из `§6.2`:

Хлебные крошки в заголовке: `"📂 programming / rust / official"` — через `render_breadcrumb`.

Секция `▸ FOLDERS (N) ──────────` — список подпапок. Каждая строка: `"󰉋  {name}/  {count} книг"`.

Секция `▸ BOOKS (N) ──────────` — список книг. Формат книги — как в существующем Book List, плюс:
- Ghost book (`NeverHadFile`): вся строка dim (nord3), иконка `󰈖`, вторая строка `"○ ghost • no file  [tags]"`
- Detached book (`Missing`): иконка `󰈖` + `⚠` оранжевым, вторая строка `"⚠ detached • file missing  last seen: 3 days ago"`

Cursor строка: стрелка `"▶"` слева.

Статус-бар внизу: `"FOLDER  path:programming/rust/official   5 books (1 ghost) ● rust"`.

Режим `[FOLDER VIEW]` показывать в заголовке справа.

### 15в. render_breadcrumb

**Файл:** `omniscope-tui/src/panels/center/breadcrumb.rs`

Функция из `§6.4`: `render_breadcrumb(frame, area, path: &[&Folder], theme)`.

Сегменты разделены `" / "` (muted стиль). Последний сегмент: `theme.fg_bright + BOLD`. Остальные: `theme.frost_mint`.

### 15г. Навигация в Folder View

Обработчик клавиш для центральной панели в режиме `FolderView` (из `§6.3`):

- `j` / `k` — переместить cursor по смешанному списку папок + книг
- `Enter` — если cursor на папке → `load_folder_contents(folder_id)`, обновить breadcrumb; если на книге → открыть в preview / во внешнем приложении
- `-` — выйти в родительскую папку (`.parent()` breadcrumb)
- `l` — войти в папку (только если cursor на папке)
- `h` — выйти в родителя (аналог `-`)
- `gg` / `G` — первый / последний элемент
- `gv` — `CenterPanelMode::BookList` (вернуться в обычный режим)
- `gb` — перейти в левую панель
- `T` — цикл `FolderViewSort`: `FoldersFirst → Mixed → BooksFirst → FoldersFirst`
- `/` — fuzzy поиск внутри текущей папки и поддиректорий

Vim операторы на папках и книгах (одинаковые клавиши, разная логика):
- `dd` — если на папке → `FolderOps::delete_folder`; если на книге → удалить книгу
- `r` — если на папке → inline rename; если на книге → rename файла
- `p` — вставить (переместить) из регистра
- `a` — создать новую папку в текущей директории

Text objects:
- `if` / `af` — все книги в папке под курсором
- `ib` / `ab` — одна книга под курсором

**Проверка:** навигация через дерево папок, Enter входит в подпапку, `-` выходит.

---

## Шаг 16. Ghost books UX

**Референс:** `FOLDER_SYSTEM.md §6.5`

**Файл:** `omniscope-tui/src/input/ghost_book_actions.rs`

Добавить операции для ghost books (доступны когда cursor на книге с `NeverHadFile`):

`gf` — "Find PDF": открыть `FindDownloadPanel` (из science модуля) с предзаполненным поиском по DOI/arXiv ID книги. Если модуль science не подключён — показать сообщение "Install omniscope-science for PDF search".

`E` — "Attach file": открыть file picker (используйте `rfd` крейт или вызвать `$EDITOR` с путём). После выбора файла: скопировать файл в директорию книги, вызвать `FolderOps::attach_file_to_ghost(book_id, file_path)`.

`@m` — "Enrich metadata": вызвать AI-обогащение через `omniscope-ai` (если подключён).

`dd` — удалить ghost book: только `db.delete_book_card(book_id)`, файл не трогать (его нет).

В preview правой панели для ghost book показывать:
- Метаданные (title, author, DOI, arXiv)
- Блок `"File not available"` с кнопкой `[Find PDF?]` (клавиша `gf`)
- Если есть DOI/arXiv → показать ссылки

Для detached books дополнительно:
- `"Last seen: {duration} ago"`
- `[Locate]` — поиск файла по hash на диске (сканировать библиотеку)
- `[Relink]` — указать новый путь вручную

**Проверка:** ghost book отображается dim, доступны операции `gf`, `E`, `dd`.

---

## Шаг 17. Sync Panel

**Референс:** `FOLDER_SYSTEM.md §7`

**Файл:** `omniscope-tui/src/panels/sync_panel.rs`

`SyncPanel` struct: `report: SyncReport`, `selected_actions: HashMap<usize, SyncAction>`, `cursor: usize`, `scroll_offset: usize`.

`SyncAction` enum: `Import`, `ImportAsGhost`, `Ignore`, `Relink(PathBuf)`, `KeepAsGhost`.

Рендер панели (ASCII-макет из `§7`):

Заголовок: `"SYNC STATUS"`, строка `"Library: ~/Books/   Last scan: {time} ago"`.

Секция `⊕ NEW (N)` — файлы на диске без карточки. Каждый элемент:
- Checkbox `[󰄬]` (отмечен) или `[ ]` (не отмечен)
- Путь файла + размер
- Если обнаружены метаданные: `"Detected: '{title}' · {author} · {year}"`
- Если arXiv update: `"arXiv: {id} (update to existing card)"`
- Если нет метаданных: `"No metadata found — needs manual review"`

Секция `⚠ DETACHED (N)` — карточки с пропавшими файлами:
- Иконка + название книги + последний известный путь
- `"Last seen: {N} days ago"`
- Кнопки `[Locate] [Relink] [Keep as ghost]`

Секция `󰉋 UNTRACKED DIRS (N)` — директории без записи в БД:
- Путь + кол-во файлов
- `[Import folder] [Ignore]`

Нижняя строка: `"[a] Apply all  [s] Apply selected  [i] Ignore  [r] Re-scan  [Esc]"`.

Клавиши:
- `j` / `k` — навигация по элементам
- `Space` / `x` — toggle checkbox (отметить для apply)
- `a` — применить все (отмеченные по умолчанию)
- `s` — применить только отмеченные
- `i` — игнорировать выбранный элемент (пометить, не показывать снова)
- `r` — повторное сканирование (`FolderSync::full_scan()`)
- `Esc` — закрыть панель

Вызов: `:sync` из command mode, `@sync` из Normal mode, автоматически при старте если обнаружены расхождения.

Метод `SyncPanel::apply_selected(&self, ops: &FolderOps, sync: &FolderSync) -> Result<()>` — применить все выбранные действия.

**Проверка:** `:sync` открывает панель, репорт корректно отображается.

---

## Шаг 18. Уведомления от Watcher в TUI

**Референс:** `FOLDER_SYSTEM.md §3.2`

**Файл:** `omniscope-tui/src/notifications.rs`

В главном event loop TUI добавить вызов `watcher.handle_next_event()` в каждой итерации.

`NotificationBar` — компонент для статус-бара с уведомлениями:
- `NotificationBar::show(message: String, duration: Duration)` — показать сообщение, автоматически скрыть через `duration`
- Мигание: первые 3 секунды — чередовать отображение каждые 500ms

При `WatcherAction::NotifyNewFile { path }`:
1. Показать в статус-баре: `"[+1 new file]"` (мигает 3 секунды)
2. Добавить `path` в `FolderState::pending_watcher_events`
3. `Space` → открыть Import Panel

При `WatcherAction::AutoImport { path }`:
1. Создать карточку через `FolderSync::auto_import_file`
2. Показать статус: `"Auto-imported: {title}"` (3 секунды)

Import Panel (`:import new` или Space по уведомлению):
```
╭─ NEW FILES DETECTED ──────────────────────────────╮
│  N new files found in ~/Books/...                  │
│                                                     │
│  [󰄬] {filename}  ({size} MB)  → {folder}/          │
│  ...                                               │
│  [a] Import all  [s] Selected  [e] Edit  [Esc]    │
╰─────────────────────────────────────────────────────╯
```

При `WatcherAction::MarkFileDetached { path }`:
1. Найти книгу по пути в БД
2. Обновить `file_presence = Missing { last_known_path: path, last_seen: Utc::now() }`
3. Обновить `FolderTree` (счётчики, если нужно)
4. Тихое уведомление в статус-баре: `"1 book detached: {title}"`

При `WatcherAction::SyncNewDirectory { path }`:
1. Если `auto_sync_dirs = true` → `FolderSync::import_directory(&path)`
2. Иначе → добавить в `pending_watcher_events`, показать уведомление

**Проверка:** скопировать PDF в директорию библиотеки → через 2 секунды уведомление в TUI.

---

## Шаг 19. AI-интеграция — папочные команды

**Референс:** `FOLDER_SYSTEM.md §11`

**Файл:** `omniscope-ai/src/actions/folder_actions.rs`

Добавить в систему AI-действий:

`RestructureFolder { folder_id: FolderId }` — AI анализирует содержимое папки и предлагает новую структуру вложенных папок. Ответ: список предлагаемых операций `Vec<ProposedFolderOp>`.

`AutoOrganize { folder_id: FolderId, apply: bool }` — реструктуризация всей библиотеки. Если `apply = false` → только preview. Preview показывается как список `mv` операций.

`TagFolderBooks { folder_id: FolderId }` — предложить теги для всех книг в папке.

`AuditFolder { folder_id: FolderId }` — проверить на дубликаты, ghost books, orphaned.

`NameFolder { folder_id: FolderId }` — предложить название на основе книг внутри.

В `LibraryMap::BookSummaryCompact` добавить поля: `"ghost": bool`, `"detached": bool`. В `LibraryMap::FolderSummary` добавить: `"ghost_count": u32`, `"physical_book_count": u32`.

Проактивные уведомления в AI: при обнаружении N ghost books по одной теме → AI предлагает `"Скачать PDF для всех?"`.

Vim-биндинги в FOLDER mode (добавить в шаг 12):
- `@p` → `RestructureFolder { folder_id: current }`
- `@t` → `TagFolderBooks { folder_id: current }`
- `@a` → `AuditFolder { folder_id: current }`

Command mode:
- `:ai restructure` → `RestructureFolder`
- `:ai auto-organize [--apply]` → `AutoOrganize`
- `:ai name-folder` → `NameFolder`
- `:ai create-folders <topic>` → создать рекомендованную структуру папок для темы

**Проверка:** `@p` в FOLDER mode → AI отвечает предложением реструктуризации.

---

## Шаг 20. Конфигурация — сохранение и загрузка

**Референс:** `FOLDER_SYSTEM.md §9`

**Файл:** `omniscope-core/src/config/folder_config.rs` (доработка шага 8)

Реализовать полный TOML-сериализатор/десериализатор для `FolderConfig` через `toml` крейт.

`FolderConfig::load(library_root: &Path) -> Result<Self>` — путь `.libr/library.toml`. Если файл не существует → `Default::default()`.

`FolderConfig::save(&self, library_root: &Path) -> Result<()>` — сериализовать в TOML и записать. Не перетирать пользовательские комментарии — использовать `toml_edit` крейт для редактирования существующего файла.

Добавить команду CLI: `omniscope config get folders.watcher.auto_import` и `omniscope config set folders.watcher.auto_import true`.

Тест roundtrip: сохранить конфиг → загрузить → все поля совпадают.

---

## Финальная проверка: E2E сценарии

После завершения всех шагов выполнить ручные сценарии из `FOLDER_SYSTEM.md §13`:

**Сценарий A — Импорт существующей коллекции:**
```bash
cd ~/TestBooks
mkdir -p programming/rust ml-papers/transformers fiction
# добавить несколько PDF файлов
omniscope init
omniscope sync --dry-run   # показывает N dirs, M files, 0 в БД
omniscope sync --strategy disk-wins
omniscope                   # TUI → gF → дерево идентично диску
```

**Сценарий B — Реструктуризация через TUI:**
```
gF → найти "misc/" → a → создать "misc/rust/" → a → создать "misc/ml/"
l → войти в misc/ → V → выбрать книги → m → переместить
Проверить: файлы физически переместились на диске
```

**Сценарий C — Ghost book:**
```bash
omniscope arxiv add 1706.03762   # без --download-pdf
# В TUI: книга отображается dim с ○ ghost
# gf → Find PDF → найдено → скачать
# Книга становится PhysicalBook
```

**Сценарий D — Watcher:**
```bash
# TUI запущен, watcher работает
cp ~/Downloads/book.pdf ~/TestBooks/programming/
# Через 2с: [+1 new file] в статус-баре
# Space → Import Panel → [Import] → карточка создана
```

**Сценарий E — Ручное перемещение файлов:**
```bash
# Вне TUI:
mv ~/TestBooks/ml-papers/transformers ~/TestBooks/ml-papers/transformers-2024
# Открыть TUI → Sync Panel автоматически
# ⚠ DETACHED (N), 󰉋 UNTRACKED DIRS (1)
# [Auto-relink by hash] → файлы привязаны
```

---

## Зависимости между шагами

```
0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9
                               ↓
                           10 (TUI state)
                           11 ← 3, 10
                           12 ← 5, 11
                           13 ← 12
                           14 ← 12
                           15 ← 3, 6, 10
                           16 ← 15
                           17 ← 6, 15
                           18 ← 7, 11
                           19 ← 5, 12, AI module
                           20 ← 8, CLI
```

Шаги 0–9 — полностью в `omniscope-core` и `omniscope-cli`, без TUI.
Шаги 10–19 — TUI-слой поверх готового core.
Шаг 20 — конфигурация, пронизывает все слои.

---

*Диск — источник правды. TUI — зеркало диска. Все операции — двусторонние.*
*Ghost-книги — полноправные участники: метаданные без файла лучше, чем ничего.*
*Никаких сюрпризов: preview + undo для каждой деструктивной операции.*
