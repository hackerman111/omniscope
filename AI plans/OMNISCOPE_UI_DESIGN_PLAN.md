# Omniscope — План разработки UI: Nord Theme

> **Философия:** Nord — это не просто цветовая схема. Это характер.
> Арктическая ясность, ледяная точность, никакого лишнего шума.
> Интерфейс должен ощущаться как хорошо написанный код: читаемый, структурированный, без украшений ради украшений.

---

## 0. Дизайн-система: Nord DNA

Прежде чем писать код — усвоить систему. Все решения вытекают отсюда.

### 0.1 Палитра

Nord делится на 4 группы. Использовать строго в рамках назначения каждой.

```toml
# ~/.config/omniscope/themes/nord.toml

# ── POLAR NIGHT (фоны, поверхности) ────────────────────────────
nord0  = "#2E3440"   # Самый тёмный фон. Панели, statusbar.
nord1  = "#3B4252"   # Вторичный фон. Активная строка, hover.
nord2  = "#434C5E"   # Третичный фон. Разделители, borders.
nord3  = "#4C566A"   # Мягкий акцент. Неактивный текст, комменты.

# ── SNOW STORM (текст) ──────────────────────────────────────────
nord4  = "#D8DEE9"   # Основной текст.
nord5  = "#E5E9F0"   # Яркий текст. Заголовки, выделения.
nord6  = "#ECEFF4"   # Белейший. Только для курсора и critical UI.

# ── FROST (интерактивные элементы, навигация) ──────────────────
nord7  = "#8FBCBB"   # Мятный. Пути, хлебные крошки.
nord8  = "#88C0D0"   # Ледяной синий. Ссылки, выделенный элемент.
nord9  = "#81A1C1"   # Синий. Ключевые слова, операторы.
nord10 = "#5E81AC"   # Тёмно-синий. Активные элементы, cursor block.

# ── AURORA (семантические цвета, статусы) ──────────────────────
nord11 = "#BF616A"   # Красный. Ошибки, удаление, danger.
nord12 = "#D08770"   # Оранжевый. Предупреждения.
nord13 = "#EBCB8B"   # Жёлтый. Метки, рейтинг (★★★).
nord14 = "#A3BE8C"   # Зелёный. Успех, "read", confirmed.
nord15 = "#B48EAD"   # Фиолетовый. AI-индикатор, magic.
```

### 0.2 Типографика в TUI

TUI не использует веб-шрифты, но типографика всё равно имеет значение.

```
Требования к шрифту терминала (пользователь настраивает сам):
  Рекомендован: JetBrains Mono Nerd Font (полный набор иконок)
  Fallback: Fira Code Nerd Font, Hack Nerd Font

Иерархия текста:
  Заголовок панели    nord6 + Bold           "LIBRARIES"
  Название книги      nord5 (активная)        "The Rust Programming Language"
                      nord4 (неактивная)
  Автор               nord3 (dim)             "Steve Klabnik, Carol Nichols"
  Мета (год, стр)     nord3 (dim)             "2022 · 560p · PDF"
  Теги                nord9 (в скобках)       [systems] [rust] [beginner]
  Рейтинг             nord13 (★)              ★★★★☆
  Статус              aurora цвет             ● reading
```

### 0.3 Символьный язык (иконки Nerd Fonts)

Единый словарь иконок. Не смешивать эмодзи и NF-иконки.

```
 󰂺  — книга (библиотека)          󰉋  — папка закрытая
 󰂺  — открытая книга              󰝰  — папка открытая
 󰌒  — тег                        󰍉  — поиск (telescope)
 󰓎  — звезда (рейтинг)            󱤅  — AI / нейросеть
 󰄬  — галочка (прочитано)         󰅖  — крестик (удаление)
 ●  — точка (статус reading)      ○  — круг (unread)
 ✓  — read                        ?  — unknown status
 ›  — стрелка навигации           …  — pending/loading
 ⎕  — буфер                       ▸  — свёрнутая группа
 ▾  — развёрнутая группа          ─  — разделитель
```

### 0.4 Пространство и геометрия

```
Минимальный размер терминала: 120×35 символов
Оптимальный: 160×45

Ширина панелей (по умолчанию):
  Левая  (библиотеки/теги):  22 символа
  Центральная (список книг): auto (заполняет остаток)
  Правая (preview):          40 символа

Отступы внутри панелей: 1 символ с каждой стороны (padding = 1)
Разделители панелей: вертикальная черта │ (nord2)
Заголовки панелей: 1 строка + нижний border ─ (nord2)
```

---

## Шаг 1. Система тем: ThemeConfig

**Файл:** `omniscope-tui/src/theme/mod.rs`

Создать `NordTheme` как главный объект конфигурации цветов.
Все цвета — только через него. Никаких хардкоженных `Color::Rgb(...)` в рендере.

```rust
pub struct NordTheme {
    // Polar Night
    pub bg:           Color,  // nord0
    pub bg_secondary: Color,  // nord1
    pub border:       Color,  // nord2
    pub muted:        Color,  // nord3

    // Snow Storm
    pub fg:           Color,  // nord4
    pub fg_bright:    Color,  // nord5
    pub fg_white:     Color,  // nord6

    // Frost
    pub frost_mint:   Color,  // nord7
    pub frost_ice:    Color,  // nord8
    pub frost_blue:   Color,  // nord9
    pub frost_dark:   Color,  // nord10

    // Aurora
    pub red:          Color,  // nord11
    pub orange:       Color,  // nord12
    pub yellow:       Color,  // nord13
    pub green:        Color,  // nord14
    pub purple:       Color,  // nord15
}

impl NordTheme {
    // Семантические алиасы — использовать в рендере, не raw цвета
    pub fn cursor_bg(&self)     -> Color { self.frost_dark  } // nord10
    pub fn cursor_fg(&self)     -> Color { self.fg_white    } // nord6
    pub fn selection_bg(&self)  -> Color { self.bg_secondary} // nord1
    pub fn active_panel(&self)  -> Color { self.frost_ice   } // nord8
    pub fn inactive_panel(&self)-> Color { self.muted       } // nord3
    pub fn tag_color(&self)     -> Color { self.frost_blue  } // nord9
    pub fn path_color(&self)    -> Color { self.frost_mint  } // nord7
    pub fn ai_color(&self)      -> Color { self.purple      } // nord15
    pub fn danger(&self)        -> Color { self.red         } // nord11
    pub fn success(&self)       -> Color { self.green       } // nord14
    pub fn warning(&self)       -> Color { self.orange      } // nord12
    pub fn star_color(&self)    -> Color { self.yellow      } // nord13
}
```

Поддержать загрузку из TOML: `~/.config/omniscope/themes/nord.toml`.
Встроенные темы: `nord` (дефолт), `catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`.
Команда `:colorscheme nord` переключает тему без перезапуска.

**Проверка:** все цвета доступны через алиасы. Хардкоженных цветов в рендере — ноль.

---

## Шаг 2. Глобальный Layout — трёхпанельная архитектура

**Файл:** `omniscope-tui/src/layout/mod.rs`

### Структура экрана

```
╔══════════════════════════════════════════════════════════════════════════╗
║  TITLEBAR (1 строка, опциональный)                                       ║
╠══════════╦══════════════════════════════════════════╦════════════════════╣
║          ║                                          ║                    ║
║  ЛЕВАЯ   ║          ЦЕНТРАЛЬНАЯ                     ║    ПРАВАЯ          ║
║  ПАНЕЛЬ  ║          (список книг)                   ║    (preview)       ║
║  (22 col)║          (auto)                          ║    (40 col)        ║
║          ║                                          ║                    ║
╠══════════╩══════════════════════════════════════════╩════════════════════╣
║  STATUSBAR (1 строка)                                                    ║
╠══════════════════════════════════════════════════════════════════════════╣
║  COMMANDLINE (1 строка, только в COMMAND/SEARCH mode)                    ║
╚══════════════════════════════════════════════════════════════════════════╝
```

Реализовать через `ratatui::layout::Layout` с `Direction::Horizontal` для панелей.
Размеры панелей — из `config.panel_sizes: [u16; 3]` (проценты).
При ширине < 80 символов: скрыть правую панель. При < 60: скрыть левую.

**Адаптивность:**
```
≥ 160 col  : три панели (22 / auto / 40)
≥ 100 col  : две панели (22 / auto), preview по Enter
< 100 col  : одна центральная, левая по Tab
```

**Граница между панелями:** одиночная вертикальная черта `│` цвет `nord2`.
Активная панель: граница меняется на `nord8` (frost_ice).

---

## Шаг 3. Левая панель — библиотеки и теги

**Файл:** `omniscope-tui/src/panels/left.rs`

### Структура

```
╭─ LIBRARIES ─────────╮
│ 󰂺  All Books    147  │   ← nord5 bold, count = nord3
│ ▾ 󰂺  Programming  89  │   ← активная, frost_ice bg
│   ├ 󰝰  rust        34  │   ← вложенная папка, nord7
│   ├ 󰝰  algorithms  12  │   ← nord4
│   └ 󰉋  systems     43  │
│ ▸ 󰂺  Fiction       31  │   ← свёрнутая
│ ▸ 󰂺  Science       27  │
│                        │
│ ─── TAGS ───────────  │   ← разделитель nord2
│  󰌒 rust           34  │   ← frost_blue
│  󰌒 async          18  │
│  󰌒 beginner       12  │
│  󰌒 systems        43  │
╰────────────────────────╯
```

**Детали рендера:**
- Активная строка: `bg = nord1`, `fg = nord5`, `bold = true`
- Неактивная: `bg = nord0`, `fg = nord4`
- Count книг: правое выравнивание, `fg = nord3`
- Дерево: отступы по 2 символа на уровень, `▾/▸` для fold
- Теги: `󰌒 ` + название + count. Цвет иконки = `nord9`
- Заголовки секций (`LIBRARIES`, `TAGS`): `nord3`, `UPPERCASE`, разделитель `─`
- Курсор в левой панели: подсвечивается `nord10` (frost_dark)

**Активная панель vs неактивная:**
- Активная: title `fg = nord8`, border `= nord8`
- Неактивная: title `fg = nord3`, border `= nord2`

---

## Шаг 4. Центральная панель — список книг

**Файл:** `omniscope-tui/src/panels/center.rs`

Это основной экран. Здесь пользователь проводит 80% времени. Каждый пиксель важен.

### Строка книги (1 строка на книгу)

```
  ▶  The Rust Programming Language   Steve Klabnik   2022  ★★★★☆  ● reading  [rust] [systems]
```

Детальная раскраска:

```
Компонент          Цвет              Примечание
─────────────────────────────────────────────────────────────────────
Маркер курсора ▶   nord8 bold        только у активной строки
Заголовок          nord5             активная строка
                   nord4             неактивная
Автор              nord3             всегда dim
Год                nord3             dim, разделитель · тоже nord3
Рейтинг ★          nord13            заполненные звёзды
Рейтинг ☆          nord2             пустые звёзды
Статус ● reading   nord8             frost_ice
Статус ○ unread    nord3             dim
Статус ✓ read      nord14            green
Теги [rust]        nord9             frost_blue, в квадратных скобках
```

### Группировка и секции

При группировке по библиотеке/автору/году — добавить заголовок секции:

```
  ─── Programming / rust (34) ────────────────────────────────────────
```
`fg = nord7` (mint), линия `─` = `nord2`

### Активная строка

```
bg = nord1
fg = nord5 (заголовок), остальное как обычно но чуть ярче
Курсорный блок ▶ = nord8
```

### Visual mode выделение

```
bg = nord10 (frost_dark)     — выделенные строки
fg = nord6 (bright white)
Отдельные книги в VISUAL: ✓ в начале строки, nord14
```

### EasyMotion overlay

При Space+Space: поверх каждой книги появляется метка:

```
  [a] The Rust Programming Language   ...
  [b] Zero to Production              ...
  [c] Programming Rust                ...
```

Метка `[a]` = `bg nord13, fg nord0, bold` (яркий жёлтый контрастный)
Остальной текст строки: `fg = nord2` (dimmed, чтобы метки читались)

### PENDING operator hint

При нажатии `d` — внизу центральной панели появляется:

```
  ┌──────────────────────────────────────────────────────┐
  │ Operator: [d] — ожидает motion или text object        │
  │ Примеры: dd  dG  dil  dat  d3j                       │
  └──────────────────────────────────────────────────────┘
```

`bg = nord1`, border `= nord9`, текст `= nord4`, примеры `= nord8`

---

## Шаг 5. Правая панель — Preview карточки

**Файл:** `omniscope-tui/src/panels/right.rs`

### Структура preview

```
╭─ PREVIEW ───────────────────────────╮
│                                      │
│  󰂺  The Rust Programming            │  ← nord5 bold, wrap
│     Language                         │
│                                      │
│  ──────────────────────────────────  │
│  󰓎  ★★★★☆                  reading  │  ← rating + status
│                                      │
│  AUTHORS                             │  ← nord3 label
│  Steve Klabnik, Carol Nichols        │  ← nord4
│                                      │
│  META                                │
│  2022 · 560 pages · PDF · EN         │
│                                      │
│  TAGS                                │
│  [rust] [systems] [beginner]         │  ← nord9
│                                      │
│  PATH                                │
│  ~/books/programming/rust/           │  ← nord7 mint
│  trlp.pdf                            │
│                                      │
│  ABSTRACT                            │
│  The Rust programming language       │  ← nord4, truncated
│  helps you write faster, more        │
│  reliable software...                │
│  (3 more lines, press i to expand)   │  ← nord3
│                                      │
│  AI SUMMARY                     󱤅  │  ← purple AI indicator
│  Memory-safe systems language...     │  ← nord4 italic
│                                      │
╰──────────────────────────────────────╯
```

**Детали:**
- Заголовок книги: `nord5`, `bold`, wrap с переносом
- Labels (`AUTHORS`, `META`, `TAGS`...): `nord3`, `UPPERCASE`, размер -1 (мелкий)
- Разделитель между rating и содержимым: `─` `nord2`
- AI summary: иконка `󱤅` = `nord15` (purple), текст = `nord4` italic
- Кавер (если есть): отрендерить через `ratatui-image` или sixel (если терминал поддерживает)
- При отсутствии файла: `fg = nord11` "No file attached"

### Inline-редактор (INSERT mode, поле)

При `cT` (change title) — правая панель не закрывается, а поле становится редактируемым:

```
  TITLE
  ┌───────────────────────────────────┐
  │ The Rust Programming Language_    │   ← fg = nord6, bg = nord1, cursor = блок nord8
  └───────────────────────────────────┘
  Esc — отмена  ·  Enter — сохранить
```

---

## Шаг 6. Статус-бар

**Файл:** `omniscope-tui/src/panels/statusbar.rs`

Статус-бар — 1 строка внизу. Разделён на зоны.

### Зоны статус-бара

```
│ 󰂺  omniscope  programming › rust │  147 books  │  [N]  │  "a  m.  ●AI  │
  ────────────────────────────────────────────────────────────────────────
  ЛЕВАЯ ЗОНА                        ЦЕНТР         РЕЖИМ   ПРАВАЯ ЗОНА
```

**Детальное описание:**

```
Левая зона (путь):
  󰂺  = nord8, "omniscope" = nord5 bold
  Разделитель ›  = nord3
  "programming" = nord7 (mint)
  › = nord3
  "rust" = nord7 bold (текущий уровень)

Центр (счётчик):
  "147 books" = nord3
  В VISUAL mode: "5 selected" = nord8 bold

Режим [N]:
  [N] NORMAL    = bg nord10, fg nord6, bold (синий блок)
  [I] INSERT    = bg nord14, fg nord0, bold (зелёный блок)
  [V] VISUAL    = bg nord12, fg nord0, bold (оранжевый блок)
  [VL]VIS-LINE  = bg nord12, fg nord0, bold
  [VB]VIS-BLK   = bg nord12, fg nord0, bold
  [:] COMMAND   = bg nord9,  fg nord6, bold
  [/] SEARCH    = bg nord13, fg nord0, bold (жёлтый)
  [g?] PENDING  = bg nord3,  fg nord5, bold (серый)

Правая зона:
  "a   — регистр pending, fg = nord13
  m.   — marks indicator, fg = nord7
  ●AI  — AI активен, fg = nord15, мигает при работе
  макрос записывается: ● REC [a] = nord11 мигающий
```

### Прогресс / Spinner

Пока AI обрабатывает — в правом конце статус-бара:

```
  ⠋ indexing... (23/147)   → ⠙ → ⠹ → ⠸ → ...
```

Spinner: braille spinner `nord15`, текст `nord3`, count `nord8`.

---

## Шаг 7. Command Line / Search Bar

**Файл:** `omniscope-tui/src/panels/cmdline.rs`

Однострочный ввод под статус-баром. Появляется только в COMMAND и SEARCH mode.

### COMMAND mode `:`

```
  : sort @author desc█
    ─────────────────────────────────────
      :sort   ← автодополнение nord8
      :search
      :split
```

Оформление:
- `:` = `nord8 bold`
- Текст ввода = `nord5`
- Курсор = блок `nord8` bg
- Автодополнение dropdown: `bg = nord1`, активный = `nord10`, текст = `nord4`
- История Up/Down: предыдущая команда появляется `fg = nord3` (dimmed)

### SEARCH mode `/`

```
  / @author:klabnik #rust y:>2020█       3 results
```

- `/` = `nord13 bold` (желтый)
- Текст = `nord5`
- Результаты в реальном времени обновляют центральную панель
- `N results` в правой части = `nord8`
- Нет результатов: `0 results` = `nord11` (red)
- Подсветка совпадений в центральной панели: `bg = nord13, fg = nord0`

---

## Шаг 8. Telescope Overlay

**Файл:** `omniscope-tui/src/overlays/telescope.rs`

Telescope — центрированный floating overlay поверх всего интерфейса.

### Структура

```
  ╭─ omniscope ──────────────────────────────────────────────────────╮
  │ /  @author:klabnik█                                    3 results │
  ├──────────────────────────────────────────────────────────────────┤
  │ ▶  The Rust Programming Language     Klabnik   2022  ★★★★☆       │
  │    Programming Rust                  Blandy    2021  ★★★☆☆       │
  │    Rust for Rustaceans               Gjengset  2021  ★★★★★       │
  ├──────────────────────────────────────────────────────────────────┤
  │                           PREVIEW                                │
  │  󰂺  The Rust Programming Language                               │
  │  Steve Klabnik, Carol Nichols · 2022 · 560p                     │
  │  [rust] [systems] [beginner]                                     │
  │                                                                  │
  │  The definitive guide to the Rust programming language...        │
  ╰──────────────────────────────────────────────────────────────────╯
  Tab — выбрать  ·  Ctrl+q — в quickfix  ·  Enter — открыть  ·  Esc — закрыть
```

**Оформление:**
- Фон overlay: `bg = nord0` с `α` прозрачностью (если терминал поддерживает)
- Задний контент: dim до `fg = nord2` (ощущение модала)
- Border: `nord9` (frost_blue)
- Title `omniscope`: `nord8 bold`
- Строка поиска: `nord5`, курсор = `nord8`
- Активный результат: `bg = nord1, fg = nord5, bold`
- Подсвеченные совпадения: `fg = nord13, bold` (желтый)
- Подсказки внизу: `nord3`, разделитель `·` = `nord2`

---

## Шаг 9. INSERT mode — форма редактирования

**Файл:** `omniscope-tui/src/overlays/edit_form.rs`

При `i` (полная форма) или `cc` — открывается floating форма поверх правой панели
(или на всю ширину в узком режиме).

### Форма

```
  ╭─ EDITING ─────────────────────────────────────────────────────╮
  │                                                                │
  │  TITLE                                              [1/7]      │
  │  ┌──────────────────────────────────────────────────────────┐  │
  │  │ The Rust Programming Language_                           │  │
  │  └──────────────────────────────────────────────────────────┘  │
  │                                                                │
  │  AUTHORS                                                       │
  │  ┌──────────────────────────────────────────────────────────┐  │
  │  │ Steve Klabnik, Carol Nichols                             │  │
  │  └──────────────────────────────────────────────────────────┘  │
  │                                                                │
  │  YEAR    LANGUAGE    PAGES       STATUS                        │
  │  ┌──────┐ ┌─────────┐ ┌────────┐ ┌──────────────────────────┐ │
  │  │ 2022 │ │ English │ │ 560    │ │ ● reading               │ │
  │  └──────┘ └─────────┘ └────────┘ └──────────────────────────┘ │
  │                                                                │
  │  TAGS                                                          │
  │  [rust] ✕  [systems] ✕  [beginner] ✕  + add tag...            │
  │                                                                │
  │  RATING                                                        │
  │  ★ ★ ★ ★ ☆   (4/5)                                             │
  │                                                                │
  ╰──Tab — след. поле  ·  Esc — сохранить  ·  Ctrl+Esc — отменить──╯
```

**Детали оформления:**
- Форма поверх контента: тень `bg = nord0` вокруг
- Border формы: `nord9`, title `[EDITING]` = `nord8 bold`
- Активное поле: border = `nord8`, `bg = nord1`
- Неактивное поле: border = `nord2`, `bg = nord0`
- Cursor в поле: блок `bg = nord8, fg = nord0`
- Label полей: `nord3 UPPERCASE`
- Прогресс `[1/7]`: `nord3`
- Теги: `bg = nord1, fg = nord9` каждый тег, `✕` = `nord11`
- `+ add tag...`: `fg = nord3 italic`
- Звёзды рейтинга: `nord13` заполненные, `nord2` пустые
- Подсказки снизу: `nord3`

### Autocomplete для тегов

```
  + add tag...
  ┌─────────────────────┐
  │ r                   │   ← ввод
  │─────────────────────│
  │ ▶ rust          34  │   ← nord8 active
  │   reliability    8  │   ← nord4
  │   reference      3  │
  └─────────────────────┘
```

`bg = nord1`, border = `nord9`, активный = `bg nord10 fg nord6`.

---

## Шаг 10. Quickfix Panel

**Файл:** `omniscope-tui/src/panels/quickfix.rs`

Открывается снизу центральной панели, занимает ~30% высоты.

```
  ╭─ QUICKFIX ─────────────────────────────── 3/12 ─────────────────╮
  │ ▶ The Rust Programming Language                    Klabnik 2022  │
  │   Zero to Production in Rust                       Palmieri 2022 │
  │   Rust for Rustaceans                              Gjengset 2021 │
  │   ...                                                            │
  ╰──cn — следующий  ·  cp — предыдущий  ·  :cdo — применить ко всем╯
```

Border = `nord12` (orange, чтобы отличался от обычных панелей).
Title = `QUICKFIX` `nord12 bold`.
Активная строка = `bg nord1`.
Count `3/12` = `nord3`.

---

## Шаг 11. Marks Overlay (`:marks`)

```
  ╭─ MARKS ────────────────────────────────────────────────────────╮
  │  Mark  Position                      Book                       │
  │  ──────────────────────────────────────────────────────────────│
  │  a     programming > rust > #3       The Rust Programming...   │
  │  b     fiction > #12                 Dune                      │
  │  '     programming > #1              Zero to Production        │
  │  <     programming > rust > #1       (visual start)            │
  │  >     programming > rust > #5       (visual end)              │
  ╰──'{char} — прыгнуть  ·  :delmarks {char} — удалить  ·  Esc——╯
```

Border = `nord7` (mint). Mark char = `nord13 bold`. Path = `nord7`. Book = `nord4`.

---

## Шаг 12. Registers Overlay (`:reg`)

```
  ╭─ REGISTERS ────────────────────────────────────────────────────╮
  │  "a   [book]  The Rust Programming Language                     │
  │  "b   [book]  Dune                                              │
  │  "0   [book]  Zero to Production (last yank)                   │
  │  "1   [book]  Programming Rust (last delete)                    │
  │  "+   [text]  /home/user/books/programming/rust/trlp.pdf        │
  │  "_   [void]  (black hole)                                      │
  ╰──"{char}p — вставить  ·  :reg {char} — фильтр  ·  Esc ─────────╯
```

Register char `"a` = `nord13 bold`. Type `[book]` = `nord9`. Content = `nord4`.

---

## Шаг 13. Help Overlay (`?` / `:help`)

**Файл:** `omniscope-tui/src/overlays/help.rs`

Cheatsheet из `VIM_MOTIONS §14` — отрендеренный красиво в TUI.

```
  ╭─ HELP — omniscope vim motions ────────────────────────────────╮
  │                                                                │
  │  НАВИГАЦИЯ         ОПЕРАТОРЫ           РЕЖИМЫ                 │
  │  j/k  вниз/вверх   d  delete           i/a/o  INSERT          │
  │  h/l  панель        y  yank             v      VISUAL          │
  │  gg/G начало/конец  c  change           V      VIS-LINE        │
  │  {/}  группы        m  move             Ctrl+v VIS-BLK         │
  │  -    вверх         p  put              :      COMMAND         │
  │                     >/<  тег            /      SEARCH          │
  │                                                                │
  │  TEXT OBJECTS       g-КОМАНДЫ           MACROS                 │
  │  il  inner library  gh  home            q{a}  запись           │
  │  al  around library gf  open file       @{a}  воспроизвести    │
  │  it  inner tag      gc  create          @@  повторить          │
  │                                                                │
  │  /:  поиск/команды  u/Ctrl+r  undo/redo  ?  help               │
  │                                                                │
  ╰──Esc — закрыть  ·  Tab — следующая секция ────────────────────╯
```

Заголовки секций: `nord8 bold`. Клавиши: `nord13` (yellow). Описания: `nord4`.
Можно листать секции Tab'ом.

---

## Шаг 14. AI Panel

**Файл:** `omniscope-tui/src/panels/ai_panel.rs`

При `@` или `<leader>a` — правая панель трансформируется в AI-чат.

```
  ╭─ 󱤅  OMNISCOPE AI ────────────────────────────────────────────╮
  │                                                                │
  │  ─── The Rust Programming Language ─────────────────────────  │
  │                                                                │
  │  Assistant                                                     │
  │  ┌──────────────────────────────────────────────────────────┐  │
  │  │ Это фундаментальная книга по Rust от официальной команды │  │
  │  │ разработчиков. Охватывает ownership, borrowing, traits,  │  │
  │  │ concurrency. Рекомендую как первую книгу по языку.       │  │
  │  └──────────────────────────────────────────────────────────┘  │
  │                                                                │
  │  Suggested actions:                                            │
  │  [1] Добавить теги  [2] Обогатить метаданные  [3] Индекс      │
  │                                                                │
  │  ┌──────────────────────────────────────────────────────────┐  │
  │  │ █                                                        │  │
  │  └──────────────────────────────────────────────────────────┘  │
  │  Ctrl+Enter — отправить  ·  @s summary  @t tags  @m metadata  │
  ╰────────────────────────────────────────────────────────────────╯
```

Header `󱤅 OMNISCOPE AI`: `nord15 bold` (purple).
Сообщение assistant: `bg = nord1`, border = `nord9`.
Suggested actions: `nord8 bold` числа, `nord4` текст.
Input: `bg = nord1`, border = `nord8`.
При работе AI: spinner `nord15` + `"Thinking..."` `nord3`.

---

## Шаг 15. Уведомления (Notifications / Toasts)

**Файл:** `omniscope-tui/src/overlays/notifications.rs`

Временные сообщения появляются в правом верхнем углу. Исчезают через 3s.

```
  ┌──────────────────────────────────┐  ← появляется slide-in (если терминал)
  │  ✓  5 books moved to 'fiction'   │  ← success: border+icon = nord14
  └──────────────────────────────────┘

  ┌──────────────────────────────────┐
  │  󱤅  AI indexed 147 books         │  ← AI: border+icon = nord15
  └──────────────────────────────────┘

  ┌──────────────────────────────────┐
  │  ⚠  File not found               │  ← warning: border+icon = nord12
  └──────────────────────────────────┘

  ┌──────────────────────────────────┐
  │  ✕  Cannot delete: 0 books match │  ← error: border+icon = nord11
  └──────────────────────────────────┘
```

Стак до 3 уведомлений одновременно. Старые вытесняются снизу вверх.
Время отображения: success 2s, warning 4s, error 6s (или до нажатия Esc).

---

## Шаг 16. Кастомизация через config.toml

**Файл:** `~/.config/omniscope/config.toml`

```toml
[ui]
theme         = "nord"              # nord | catppuccin-mocha | gruvbox-dark | tokyo-night
panel_sizes   = [22, 0, 40]        # 0 = auto
show_preview  = true
show_icons    = true                # требует Nerd Font
show_titlebar = false               # дополнительная строка сверху
min_width     = 120                 # при меньшей ширине — адаптировать

[ui.statusbar]
show_path     = true
show_count    = true
show_mode     = true
show_ai       = true
show_marks    = true
show_registers = true

[ui.animations]
enabled       = true                # spinners, transitions
spinner_style = "braille"           # braille | dots | line | classic

[ui.book_list]
show_author   = true
show_year     = true
show_rating   = true
show_status   = true
show_tags     = true
max_tags      = 3                   # показывать максимум N тегов в строке
tag_style     = "brackets"          # brackets [tag] | hash #tag | plain tag

[ui.preview]
show_abstract = true
show_ai_summary = true
abstract_lines = 5                  # сколько строк abstract показывать
show_cover    = true                # если терминал поддерживает изображения
```

---

## Шаг 17. Адаптация для разных терминалов

**Файл:** `omniscope-tui/src/term/capabilities.rs`

При старте определить возможности терминала:

```rust
pub struct TermCapabilities {
    pub true_color:     bool,   // 24-bit RGB
    pub unicode:        bool,   // полный Unicode
    pub nerd_fonts:     bool,   // определить через конфиг или env
    pub sixel:          bool,   // изображения (для обложек)
    pub kitty_graphics: bool,   // kitty protocol
    pub bracketed_paste: bool,
    pub mouse:          bool,
}
```

**Fallback цветовые схемы:**

```
true_color = true  : полная Nord (#2E3440 и т.д.)
256 colors         : ближайшие ANSI-256 эквиваленты Nord
16 colors          : базовая Nord-совместимая схема на системных цветах
```

**Fallback иконки:**

```
Nerd Fonts = true : 󰂺 󰌒 󰝰 󰉋 󱤅
Nerd Fonts = false: [L] [T] [F] [D] [AI]  ← ASCII-альтернативы
```

Переменные окружения для override:
```
OMNISCOPE_THEME=nord
OMNISCOPE_NO_ICONS=1
OMNISCOPE_NO_COLOR=1
COLORTERM=truecolor          # стандартный детект
```

---

## Шаг 18. Производительность рендера

**Файл:** `omniscope-tui/src/render/mod.rs`

Все UI-оптимизации, которые нельзя откладывать.

```
□ Dirty-флаг: перерисовывать только изменившиеся области
□ Виртуализированный список: в памяти только viewport ±50 строк
□ Дебаунс ввода: 16ms (1 кадр) для не-критичных обновлений
□ Lazy preview: правая панель загружается только когда видима
□ Spinner: обновление только своей области, не весь экран
□ Бенчмарк: cargo flamegraph на прокрутке 10 000 книг
□ Цель: каждый кадр < 16ms (60fps эквивалент для TUI)
```

---

## Шаг 19. Финальная полировка: Nord как характер

Последний шаг — аудит всего интерфейса на соответствие Nord-эстетике.

**Чеклист:**

```
□ Ни одного хардкоженного цвета — только ThemeConfig
□ Все границы — одиночная линия ╭─╮ / ├─┤ / ╰─╯ (rounded corners где возможно)
□ Padding внутри каждой панели: ровно 1 символ
□ Нет "мусорных" символов — каждый символ несёт смысл
□ Цветовая семантика соблюдена: nord14 ТОЛЬКО для success, nord11 ТОЛЬКО для danger
□ Активная панель всегда отличима от неактивной (border nord8 vs nord2)
□ Режим всегда читаем в статус-баре — даже при минимальной ширине
□ Spinner никогда не блокирует ввод
□ Escape из любого overlay — возвращает в точно то же состояние
□ При отсутствии Nerd Fonts — интерфейс деградирует красиво, не ломается
```

**Nord character test:** открыть omniscope в тёмной комнате.
Если интерфейс выглядит как арктический пейзаж — холодный, чистый, структурированный — задача выполнена.

---

## Итоговая карта зависимостей

```
Шаг 0  — фундамент (ThemeConfig, цветовая система)
Шаг 1  → зависит от Шага 0
Шаг 2  → зависит от Шага 0 (layout до контента)
Шаги 3,4,5 → зависят от Шага 2 (панели внутри layout)
Шаг 6  → зависит от Шагов 3,4,5 (знает о всех панелях)
Шаг 7  → зависит от Шагов 2,6
Шаги 8,9,10,11,12,13,14 → overlays, зависят от Шага 2 + данных
Шаг 15 → зависит от Шага 2 (позиционирование toast)
Шаг 16 → независим, но должен быть до Шага 17
Шаг 17 → зависит от Шага 1 (тема) и Шага 16 (конфиг)
Шаг 18 → финальный аудит производительности, зависит от всего
Шаг 19 → финальный аудит эстетики, зависит от всего
```

---

*Nord — это язык холодной ясности. Каждый цвет — на своём месте. Каждый элемент — необходим.*
