# Omniscope

TUI-менеджер библиотеки для книг и научных статей. Работает в терминале, управляется с клавиатуры как Vim, умеет работать с DOI/arXiv/BibTeX.

Grounded in простой идее: хорошая библиотечная программа не должна требовать Electron.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status: Active Development](https://img.shields.io/badge/Status-Active%20Development-yellow.svg)]()

---

## Что это

Трёхпанельный TUI с Vim-управлением и встроенным AI-агентом, который знает вашу коллекцию. Альтернатива Zotero для тех, кто проводит в терминале больше времени, чем в браузере.

---

## Статус

Проект в активной разработке. Готово ядро (**Phase 1: Core TUI**).

**Работает сейчас:**
- Трёхпанельный интерфейс (Библиотеки/Теги — Список — Предпросмотр) на `ratatui`
- Vim-режимы: `NORMAL`, `INSERT`, `VISUAL`, `COMMAND`
- Базовая навигация: `j/k`, `gg/G`, `Ctrl+d/u`
- CLI-скелет для скриптов
- Холодный старт < 100ms

**В разработке (Phase 1–2):**
- Полные Vim-операторы: `d`, `y`, `c` с text objects (`iw`, `il`, `it`)
- Registers, Marks, Macros
- Fuzzy-поиск с DSL: `@author:knuth #algorithms year:>2020`
- Frecency-ранжирование

**Планируется (Phase 3–4):**
- Научный модуль: arXiv, DOI/ISBN, citation graph, BibTeX
- AI-агент: семантический поиск по содержимому, проактивные советы, JSON-команды для управления приложением
- Локальные модели через Ollama

---

## Стек

| Компонент | Технологии |
|---|---|
| Язык | Rust |
| Интерфейс | Ratatui, Crossterm |
| База данных | SQLite (sqlx) |
| Поиск | Tantivy (FTS), Usearch (Vector) |
| Синхронизация | Axum |
| AI | Ollama, Anthropic API, OpenAI API |

---

## Документация

- [`OMNISCOPE_MASTER_PLAN.md`](OMNISCOPE_MASTER_PLAN.md) — дорожная карта
- [`Omniscope_VIM_MOTIONS.md`](Omniscope_VIM_MOTIONS.md) — спецификация Vim-управления
- [`Omniscope_AI_SYSTEM.md`](Omniscope_AI_SYSTEM.md) — архитектура AI-агента
- [`Omniscope_SCIENCE.md`](Omniscope_SCIENCE.md) — научные функции
