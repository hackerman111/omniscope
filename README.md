# Omniscope

Terminal-first менеджер библиотеки книг и научных статей с Vim-управлением, быстрым поиском и научным модулем (DOI/arXiv/citations).

[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/badge/Release-v0.1.0--alpha-yellow.svg)]()

## Alpha status

Текущий релиз: **v0.1.0-alpha** (alpha 0.1).

Проект уже пригоден для ежедневной работы в терминале, но API и UX продолжают активно меняться.  
Фокус текущего цикла: стабилизация научного модуля и шлифовка TUI.

## Что уже есть

### TUI + Vim grammar

- Трёхпанельный интерфейс: библиотеки/папки, список, предпросмотр.
- 7 режимов: `Normal`, `Insert`, `Visual`, `Command`, `Search`, `Pending`, popup modes.
- Грамматика команд в стиле Vim: `[count][operator][motion/text-object]`.
- Макросы, регистры, marks, quickfix, undo/redo, command-line режим.
- Контекстные hints и обновлённый help-экран.

### Поиск и навигация

- Fuzzy-поиск (`nucleo`) в Telescope-подобном overlay.
- DSL-фильтры по полям (`@author:`, `#tag`, `year:`, и т.д.).
- Frecency-ранжирование.
- Быстрые перемещения (`f/F/t/T`, EasyMotion).

### Folder system

- Локальная библиотека в стиле `.git`: каталог `.libr/` рядом с файлами.
- Сканирование, синхронизация файлов и карточек, scaffold шаблонов папок.
- Поддержка ghost cards (карточек без физического файла).

### Scientific module (реализованная часть)

- Типизированные идентификаторы: DOI, arXiv, ISBN + извлечение из текста/PDF.
- Источники метаданных: CrossRef, Semantic Scholar, OpenAlex, Unpaywall, Open Library, CORE.
- Пайплайн обогащения метаданных (локально + онлайн), merge по приоритетам источников.
- Извлечение references из PDF, попытка резолва, citation graph поля.
- Форматы: BibTeX, RIS, CSL citation.
- Дедупликация и merge карточек.
- TUI-панели: `References`, `Citation Graph`, `Find & Download`, `Article Card`.
- Научные команды в command mode (`:refs`, `:cited-by`, `:cite`).


[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/badge/Release-v0.1.0--alpha-yellow.svg)]()

## Alpha status

Текущий релиз: **v0.1.0-alpha** (alpha 0.1).

Проект уже пригоден для ежедневной работы в терминале, но API и UX продолжают активно меняться.  
Фокус текущего цикла: стабилизация научного модуля и шлифовка TUI.

## Что уже есть

### TUI + Vim grammar

- Трёхпанельный интерфейс: библиотеки/папки, список, предпросмотр.
- 7 режимов: `Normal`, `Insert`, `Visual`, `Command`, `Search`, `Pending`, popup modes.
- Грамматика команд в стиле Vim: `[count][operator][motion/text-object]`.
- Макросы, регистры, marks, quickfix, undo/redo, command-line режим.
- Контекстные hints и обновлённый help-экран.

### Поиск и навигация

- Fuzzy-поиск (`nucleo`) в Telescope-подобном overlay.
- DSL-фильтры по полям (`@author:`, `#tag`, `year:`, и т.д.).
- Frecency-ранжирование.
- Быстрые перемещения (`f/F/t/T`, EasyMotion).

### Folder system

- Локальная библиотека в стиле `.git`: каталог `.libr/` рядом с файлами.
- Сканирование, синхронизация файлов и карточек, scaffold шаблонов папок.
- Поддержка ghost cards (карточек без физического файла).

### Scientific module (реализованная часть)

- Типизированные идентификаторы: DOI, arXiv, ISBN + извлечение из текста/PDF.
- Источники метаданных: CrossRef, Semantic Scholar, OpenAlex, Unpaywall, Open Library, CORE.
- Пайплайн обогащения метаданных (локально + онлайн), merge по приоритетам источников.
- Извлечение references из PDF, попытка резолва, citation graph поля.
- Форматы: BibTeX, RIS, CSL citation.
- Дедупликация и merge карточек.
- TUI-панели: `References`, `Citation Graph`, `Find & Download`, `Article Card`.
- Научные команды в command mode (`:refs`, `:cited-by`, `:cite`).

>>>>>>> e1e9cf91aa6d68d4aaf5e42765714ffe83f7b1a7
### CLI

- CRUD по карточкам, теги, библиотеки, папки, статистика, диагностика.
- `init/scan/sync` для локальной библиотеки.
- JSON output mode для автоматизации и AI-агентов (`--json`, `OMNISCOPE_JSON=1`).

## Архитектура workspace

`Omniscope` — Rust workspace из 7 крейтов:

- `crates/omniscope-core` — модели, storage, бизнес-логика.
- `crates/omniscope-tui` — интерфейс на `ratatui` + `crossterm`.
- `crates/omniscope-cli` — CLI над core/tui/science.
- `crates/omniscope-science` — scientific enrichment, references, formats.
- `crates/omniscope-ai` — AI-слой (архитектура и базовые заготовки).
- `crates/omniscope-server` — server/sync слой (в разработке).
- `crates/omniscope-ffi` — FFI-интеграции (в разработке).

## Быстрый старт

### 1) Зависимости

- Rust toolchain (stable).
- SQLite (используется через `rusqlite` bundled).
- Для полного PDF-обогащения желательно иметь: `pdftotext`, `pdfinfo`, `qpdf`, `mutool`.

### 2) Сборка

```bash
cargo build
```

### 3) Инициализация библиотеки

```bash
cargo run -p omniscope-cli -- init . --name "My Library" --create-dir
```

### 4) Запуск TUI

```bash
cargo run -p omniscope-cli --
```

### 5) Первичный импорт

```bash
cargo run -p omniscope-cli -- import ./books --recursive
```

## Примеры CLI

```bash
# Поиск
cargo run -p omniscope-cli -- search "deep learning"

# Список книг
cargo run -p omniscope-cli -- list --limit 100

# Скан диска и автосоздание карточек
cargo run -p omniscope-cli -- scan --auto-create-cards

# Синхронизация карточек в БД
cargo run -p omniscope-cli -- sync

# Диагностика окружения
cargo run -p omniscope-cli -- doctor
```

## Куда проект идёт (из AI plans)

Ниже дорожная карта по фазам, зафиксированная в `AI plans/OMNISCOPE_MASTER_PLAN.md`:

| Версия | Фокус |
|---|---|
| `v0.1.x` | TUI ядро, Vim grammar, CRUD, folder system, scientific foundation |
| `v0.2.0` | Поиск/DSL/frecency стабилизация и расширение |
| `v0.3.0` | Полный научный модуль: arXiv/Sci-Hub/метаданные/citation workflows |
| `v0.4.0` | Omniscope AI: Action Protocol, память, провайдеры, MCP |
| `v0.5.0` | Импорт/экспорт экосистемы (Zotero, Calibre, форматы) |
| `v0.6.0` | Сервер + синхронизация между устройствами |
| `v1.0.0` | Полировка UX, документация, плагины, production hardening |

По научному плану в `AI plans/OMNISCOPE_SCIENCE_IMPL_PLAN.md` уже реализован большой блок шагов (основа + TUI-сцены); следующие этапы — CLI-расширения, статистика и интеграция с `omniscope-ai`.

## Документация

- [Master plan](AI%20plans/OMNISCOPE_MASTER_PLAN.md)
- [Science spec](AI%20plans/Omniscope_SCIENCE.md)
- [Science implementation plan](AI%20plans/OMNISCOPE_SCIENCE_IMPL_PLAN.md)
- [AI system architecture](AI%20plans/Omniscope_AI_SYSTEM.md)
- [Vim motions spec](AI%20plans/Omniscope_VIM_MOTIONS.md)
- [UI design plan](AI%20plans/OMNISCOPE_UI_DESIGN_PLAN.md)
- [Storage and folder system](AI%20plans/OMNISCOPE_STORAGE.md), [folder plan](AI%20plans/OMNISCOPE_FOLDER_SYSTEM.md)

## Проверка качества

```bash
cargo fmt --all
cargo test
```

При наличии `clippy`:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Лицензия

MIT — см. файл `LICENSE`.
