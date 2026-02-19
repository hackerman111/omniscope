# 🔭 Omniscope — Статус проекта vs Мастер-план

> Анализ на 2026-02-19. Исходники: `crates/` (3 крейта из 7 запланированных).

---

## Что есть сейчас

| Крейт | Статус |
|---|---|
| `omniscope-core` | ✅ Активно разработан |
| `omniscope-tui` | ✅ Активно разработан |
| `omniscope-cli` | ✅ Базовый CLI готов |
| `omniscope-ai` | ❌ Отсутствует |
| `omniscope-science` | ❌ Отсутствует |
| `omniscope-server` | ❌ Отсутствует |
| `omniscope-ffi` | ❌ Отсутствует |

---

## ФАЗА 0: Основание

**Статус: ~60% завершена**

| Задача | Статус |
|---|---|
| Cargo workspace (core, tui, cli) | ✅ Есть, 3 из 7 крейтов |
| `profile.release` / `profile.dev` оптимизации | ❓ Не проверялось ([Cargo.toml](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-cli/Cargo.toml) workspace не читался) |
| GitHub Actions / CI | ❓ Не обнаружено `.github/` |
| `pre-commit` hooks | ❓ Не обнаружено |
| `cargo-criterion` бенчмарки | ❌ Нет |
| **BookCard** — полная структура | ✅ Реализована в [models/book.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/models/book.rs) |
| ScientificIdentifiers (DOI, ArxivId, ISBN) | ❌ Нет — только `isbn: Vec<String>` в метаданных |
| OmniscopeAction enum | ❌ Нет |
| DocumentType таксономия | ❌ Нет |
| **BookSummaryView** | ✅ Реализована |
| LibraryMap структура | ❌ Нет |
| ActionLogEntry | ❌ Нет |
| UserProfile | ❌ Нет |
| JSON Schema через schemars | ❌ Нет |
| Unit-тесты roundtrip | ✅ Есть в [models/book.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/models/book.rs) |
| **SQLite: таблица books** | ✅ Есть ([database.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs)) |
| SQLite: таблицы tags, libraries | ✅ Частично (поля есть, отдельная таблица tags не используется) |
| SQLite: folders, action_log, frecency | ❌ Нет отдельных таблиц — frecency как колонка в [books](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs#186-225) |
| FTS5 virtual table `books_fts` | ✅ Реализована |
| Индексы по полям | ❌ Нет явных `CREATE INDEX` |
| SQLite миграции (sqlx migrate) | ❌ Используется rusqlite, не sqlx |
| **XDG-пути** | ✅ `~/.config/omniscope/`, `~/.local/share/omniscope/` |
| Атомарная запись (write-to-temp + rename) | ❓ Надо проверить [json_cards.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/json_cards.rs) — скорее нет |
| **config.toml** через figment | ❌ Используется toml (не figment) |
| ENV override (OMNISCOPE_CONFIG) | ✅ Реализован |
| Тест config merging | ✅ Roundtrip-тест есть |
| **Бинарник omniscope с clap** | ✅ Есть |
| `--json`, `--version`, [help](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-tui/src/app.rs#550-554) | ✅ Реализованы |
| Стандарт JSON-вывода `{status, data, meta}` | ⚠️ Частично — `meta.duration_ms` отсутствует |
| Exit codes (0/1/2/3) | ⚠️ Только 0 и 8 найдено |
| OMNISCOPE_JSON=1, OMNISCOPE_LIBRARY_PATH | ❌ Нет |
| **ratatui App loop** | ✅ Есть в `omniscope-tui` |
| Panic handler | ❓ Не проверялось |
| OMNISCOPE_TIMING=1 | ❌ Нет |
| Холодный старт < 100ms | ❓ Не замерялось |

---

## ФАЗА 1: Ядро TUI

**Статус: ~45% завершена**

| Задача | Статус |
|---|---|
| Layout: 3 панели (sidebar + book list + preview) | ✅ Реализован в [app.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-tui/src/app.rs) |
| Настраиваемые размеры панелей | ✅ `panel_sizes` в config |
| Статус-бар с режимом | ✅ Есть (mode отображается) |
| Цветовые темы (catppuccin-mocha, gruvbox, tokyo-night) | ⚠️ Конфиг есть, реализация темы — не подтверждена |
| Nerd Font иконки | ❓ |
| Рендеринг < 16ms | ❓ Не замерялось |
| **VirtualizedBookList** | ❌ Нет — загружается до 500 книг сразу ([list_books(500, 0)](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs#186-225)) |
| Prefetch при прокрутке | ❌ Нет |
| **Mode state machine** (7 режимов) | ⚠️ 5 режимов: Normal/Insert/Search/Command/Visual (нет Visual-Line, Visual-Block) |
| NORMAL: j/k/h/l/gg/G | ✅ Реализовано |
| NORMAL: Ctrl+d/u/f/b | ❓ Не подтверждено |
| NORMAL операторы d/y/c/m | ⚠️ Частично (dd = delete, yp = yank path) |
| PENDING mode (ожидание motion) | ❌ Нет |
| Text objects (il/al/it/at/ia) | ❌ Нет |
| EasyMotion (Space+Space) | ❌ Нет |
| Counts (3dd, 5j) | ❌ Нет |
| Marks (m{a-z}, '{a-z}) | ❌ Нет |
| Registers | ❌ Нет |
| COMMAND (:q :w :wq :qa) | ⚠️ Частично (парсинг команды есть, реализация — не все) |
| keybindings.toml | ❌ Нет |
| Leader key | ❌ Нет |
| **Добавить книгу (форма)** | ✅ `AddBookForm` с 6 полями |
| Автоопределение формата (libmagic FFI) | ❌ Используется расширение файла |
| Извлечение метаданных PDF/EPUB (poppler/libepub FFI) | ❌ Нет — только парсинг имени файла |
| Wizard (пошаговая форма) | ⚠️ Простая форма есть |
| Редактирование карточки | ⚠️ Частичное (теги, рейтинг, статус) |
| Быстрые изменения cT/ca/cy | ❌ Нет |
| **dd (удаление)** | ✅ Есть |
| dD (карточка + файл), d_ (только файл) | ❌ Нет |
| **Undo/Redo (u/Ctrl+r)** | ❌ Нет |
| Открыть файл в системном приложении | ✅ `viewer::open_book()` |
| **Создание/переименование библиотек и тегов** | ⚠️ Только создание через поле в форме |
| Дерево папок | ❌ Нет |
| Перемещение книг между библиотеками | ❌ Нет |
| **Фильтр sidebar по библ/тегу** | ✅ Реализован |
| Сортировка | ❌ Нет |
| **CLI: omniscope book add/get/update/delete/list** | ✅ Реализовано |
| CLI: omniscope book tag add/remove | ✅ Есть |
| CLI: omniscope book file / note | ❌ Нет |
| CLI: omniscope tag list/create/delete | ⚠️ Только list |
| CLI: omniscope library list/create | ⚠️ Только list |
| CLI: omniscope folder / config / doctor | ✅ doctor есть; folder/config — нет |
| Бенчмарк CI < 150ms | ❌ Нет |

---

## ФАЗА 2: Поиск

**Статус: ~35% завершена**

| Задача | Статус |
|---|---|
| **Telescope Overlay** | ✅ Реализован (`TelescopeState` в popup.rs) |
| DSL autocomplete в telescope | ✅ Есть (candidates + chips) |
| Множественный выбор (Tab, Ctrl+q) | ❌ Нет |
| Quickfix list | ❌ Нет |
| Debounce 50ms | ❓ Не проверялось |
| **Fuzzy поиск (nucleo)** | ✅ Реализован в [search.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/search.rs) |
| SQLite FTS5 поиск | ✅ [search_fts()](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs#245-287) в [database.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs) |
| Merge & Rank (RRF) | ❌ Нет — fuzzy и FTS используются раздельно |
| OMNISCOPE_PERF=1 трассировка | ❌ Нет |
| **DSL парсер** | ✅ Полностью реализован в [search_dsl.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/search_dsl.rs) |
| @author, #tag, y:, r:, s:, f:, lib:, has: | ✅ Все реализованы |
| NOT оператор | ✅ Есть |
| `omniscope search --dsl` | ⚠️ Только FTS через `omniscope search`, без DSL-флага |
| **Frecency** (zoxide-like) | ✅ [record_access()](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs#408-425) в database, [frecency.rs](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/frecency.rs) |
| Алгоритм frecency с весами | ⚠️ Базовая версия (не все веса из плана) |
| :g/{pattern}/{command} глобальные команды | ❌ Нет |
| Макросы q{a}/@{a} | ❌ Нет |
| `omniscope search --mode fuzzy/fts/semantic` | ❌ Нет |

---

## ФАЗЫ 3–7

**Статус: 0% — не начаты**

| Фаза | Крейт | Статус |
|---|---|---|
| Фаза 3: Научный модуль | `omniscope-science` | ❌ Крейт не создан |
| Фаза 4: Omniscope AI | `omniscope-ai` | ❌ Крейт не создан |
| Фаза 5: Импорт/Экспорт | `omniscope-science` | ❌ |
| Фаза 6: Сервер | `omniscope-server` | ❌ Крейт не создан |
| Фаза 7: Полировка | все | ❌ |

---

## Итоговая оценка

```
Фаза 0 (Основание)    ███████░░░░░  ~60%
Фаза 1 (Ядро TUI)     █████░░░░░░░  ~45%
Фаза 2 (Поиск)        ████░░░░░░░░  ~35%
Фаза 3 (Наука)        ░░░░░░░░░░░░   0%
Фаза 4 (AI)           ░░░░░░░░░░░░   0%
Фазы 5-7              ░░░░░░░░░░░░   0%
```

**Версия по плану:** ~v0.1.0 (между v0.1 и v0.2)

---

## Топ-приоритетные задачи для завершения Фазы 1

1. **VirtualizedBookList** — сейчас грузится 500 книг, нарушает SLA
2. **Undo/Redo (u / Ctrl+r)** — указан как обязательный
3. **Vim counts** (3dd, 5j) и PENDING mode
4. Атомарная запись карточек (write-to-temp + rename)
5. Миграции SQLite (сейчас [init_schema](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-core/src/storage/database.rs#31-96) не versioned)
6. `FolderHeader` в sidebar заглушка — нет реальных папок
7. Panic handler в TUI

## Критические несоответствия стеку

| Требование по плану | Реализовано |
|---|---|
| `sqlx` (async SQLite) | ❌ Использует `rusqlite` (синхронный) |
| `figment` (конфиг) | ❌ Использует [toml](file:///home/papayka/Documents/AMI/Omniscope/crates/omniscope-cli/Cargo.toml) напрямую |
| `tokio` (async runtime) | ❓ Не обнаружен в крейтах |
| `tantivy` (full-text) | ❌ Используется SQLite FTS5 вместо tantivy |
| libmagic FFI | ❌ Только extension detection |
| poppler/libepub FFI | ❌ Нет |
