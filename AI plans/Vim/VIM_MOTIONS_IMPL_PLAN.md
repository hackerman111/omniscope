# Пошаговый план реализации Vim Motions — для ИИ агента

> **Контекст:** omniscope — менеджер библиотеки на Rust (ratatui + crossterm).
> Vim motions — это не набор хоткеев, а грамматика: `[count][verb][noun]`.
> Этот план рассчитан на последовательное выполнение агентом без пропуска шагов.

---

## Принципы работы агента

Перед каждым шагом — читай соответствующий раздел `VIM_MOTIONS.md`.
После каждого шага — запускай тесты и бенчмарки. Не переходи дальше, если тесты красные.
Каждый шаг — это атомарный коммит с осмысленным сообщением.

---

## Шаг 0. Подготовка: фундамент перед vim

**Файлы:** `omniscope-tui/src/input/mod.rs`, `omniscope-tui/src/state/mod.rs`

**Что сделать:**

Создать базовый event loop в ratatui — он должен принимать `crossterm::event::Event`
и передавать в обработчик. Пока никакой логики — только скелет.

Определить центральный `AppState` с полями:

```rust
pub struct AppState {
    pub mode: Mode,
    pub pending_operator: Option<Operator>,
    pub pending_count: Option<u32>,
    pub cursor: CursorPos,
    pub jump_list: JumpList,
    pub registers: RegisterMap,
    pub marks: MarkMap,
    pub macro_recorder: MacroRecorder,
}
```

**Проверка:** `cargo build` без предупреждений.

---

## Шаг 1. State machine режимов

**Файл:** `omniscope-tui/src/input/mode.rs`
**Референс:** `VIM_MOTIONS §1`

Реализовать enum с 7 вариантами:

```rust
pub enum Mode {
    Normal,
    Insert,
    Visual(VisualKind),   // Char / Line / Block
    Command,
    Search,
    Pending(Operator),    // Ожидание второй клавиши
}
```

Реализовать все переходы: `Esc/Ctrl+C` → Normal из любого режима,
`i/a/o` → Insert, `v/V/Ctrl+v` → Visual*, `:` → Command, `//?` → Search.

Визуальный индикатор в статус-баре: `[N]` синий, `[I]` зелёный, `[V]` оранжевый и т.д.
При входе в Pending показывать подсказку: `"Operator: [d] — ожидает motion или text object"`.

**Тест:** юнит-тест каждого перехода. `Mode::from_key(key) -> Option<Mode>`.

---

## Шаг 2. Парсинг count-префикса

**Файл:** `omniscope-tui/src/input/count.rs`

Числа, нажатые в Normal mode перед командой, накапливаются в `pending_count: Option<u32>`.
`0` в начале — это не count, это команда (перейти к началу строки / первой книге).
Любой не-цифровой ввод — применить накопленный count к следующей команде и сбросить.
Максимальный count — 9999 (защита от зависания).

**Тест:** `"3" → "j"` → `count=3, motion=Down`. `"0"` → `count=None, action=GotoFirst`.

---

## Шаг 3. Базовая навигация NORMAL mode (j/k/gg/G)

**Файл:** `omniscope-tui/src/input/normal.rs`
**Референс:** `VIM_MOTIONS §2.1`

Это самый частый путь выполнения — оптимизировать в первую очередь.

Реализовать движения:
- `j/k` — следующая/предыдущая книга, с count: `5j` = 5 книг вниз
- `gg` — к первой книге; `G` — к последней; `[N]G` — к N-й
- `H/M/L` — High/Middle/Low (первая/средняя/последняя видимая)
- `Ctrl+d/u` — пол-экрана; `Ctrl+f/b` — полный экран; `Ctrl+e/y` — скролл без движения курсора

Все навигационные команды должны добавлять позицию в **jump list** (кроме j/k одиночных).

**Бенчмарк:** `j` должна отрабатывать < 5ms на списке из 10 000 книг.

---

## Шаг 4. Горизонтальная навигация и иерархия

**Референс:** `VIM_MOTIONS §2.1` (горизонтальная навигация, прыжки по иерархии)

- `h/l` — переход между панелями (левая ↔ центральная ↔ правая)
- `Ctrl+h/l` — переход всегда, независимо от текущего фокуса
- `Tab/Shift+Tab` — следующая/предыдущая панель циклически
- `-/Backspace` — выйти на уровень выше (папка → библиотека → All)
- `Enter` — войти в папку или открыть книгу
- `gp` — к родителю, `gr` — к корню, `gh` — к "All Books"
- `{/}` — предыдущая/следующая группа; `[[/]]` — предыдущая/следующая библиотека

---

## Шаг 5. Jump List (Ctrl+o / Ctrl+i)

**Файл:** `omniscope-tui/src/input/jump_list.rs`
**Референс:** `VIM_MOTIONS §2.1` (jump list секция)

`JumpList` — вектор позиций `(library, folder, book_id)` с указателем текущей позиции.
Добавлять в список: `gg`, `G`, `[[`, `]]`, `gp`, `gr`, поиск, `''`.
`Ctrl+o` — назад, `Ctrl+i` — вперёд. `''` — к предыдущей позиции (последнее двойное нажатие).
Максимум 100 записей (FIFO при переполнении).

**Тест:** серия переходов → Ctrl+o×3 → должен вернуться на 3 позиции назад.

---

## Шаг 6. Операторы и PENDING mode

**Файл:** `omniscope-tui/src/input/operator.rs`
**Референс:** `VIM_MOTIONS §2.2`

Операторы `d/y/c/m/p/>/<` при нажатии в Normal mode переводят в `Mode::Pending(operator)`.
В Pending mode следующая клавиша — это motion или text object.
Удвоение (`dd`, `yy`, `cc`) — применить к текущей книге.

```rust
pub enum Operator { Delete, Yank, Change, Move, Put, AddTag, RemoveTag, Normalize }
```

Карта `c`-подкоманд: `cT` title, `ca` author, `cy` year, `ct` tags, `cs` status, `cr` rating.
Карта `m`-подкоманд: `ml` → library picker, `mf` → folder picker, `mt` → retag.

**Тест:** `d` → mode == Pending(Delete). `dd` → DeleteCurrentBook. `d3j` → DeleteNextN(3).

---

## Шаг 7. Text Objects

**Файл:** `omniscope-tui/src/input/text_object.rs`
**Референс:** `VIM_MOTIONS §2.3`

Text objects: после нажатия `i` или `a` в Pending mode ожидается объект.

```
i/a + b → book (текущая)
i/a + l → library (все книги в библиотеке)
i/a + t → tag (все книги с текущим тегом)
i/a + f → folder
i/a + a → author (все книги этого автора)
i/a + y → year
```

`i` = "inner" (только содержимое), `a` = "around" (вместе с контейнером).
Пример: `dal` — удалить библиотеку целиком, `dil` — только книги внутри.

**Тест:** `dil` на библиотеке из 5 книг → 5 книг удалены, библиотека осталась.

---

## Шаг 8. g-команды (g-prefix)

**Файл:** `omniscope-tui/src/input/g_commands.rs`
**Референс:** `VIM_MOTIONS §2.4`

После нажатия `g` — ожидать вторую клавишу (PENDING с таймаутом 1s).

Реализовать: `gh` (home), `gl` (last pos), `gb` (buffers), `gB` (prev buffer),
`gf` (open file in OS), `gF` (open folder), `gp` (parent), `gr` (root),
`gc` (create submenu: b/l/f/t), `gI` (open in $EDITOR), `gv` (reselect visual),
`gz` (center view), `g/` (search с историей), `g*` (поиск автора под курсором).

---

## Шаг 9. z-команды

**Файл:** `omniscope-tui/src/input/z_commands.rs`
**Референс:** `VIM_MOTIONS §2.5`

- `zz` — центрировать текущую книгу в viewport
- `zt` — к верху, `zb` — к низу
- `za/zo/zc` — toggle/open/close fold (группу/папку)
- `zR/zM` — развернуть/свернуть все группы
- `zi` — toggle folding целиком

---

## Шаг 10. Marks

**Файл:** `omniscope-tui/src/input/marks.rs`
**Референс:** `VIM_MOTIONS §2.6`

`MarkMap` — HashMap от `char` к `BookPosition`.
- `m{a-z}` — установить локальную метку
- `m{A-Z}` — глобальная метка (между библиотеками)
- `'{a-z/A-Z}` — прыгнуть к метке
- `''` — к последней позиции перед прыжком
- `:marks` — показать все метки
- `:delmarks {a-z}` — удалить метку

Автоматические метки: `'<` и `'>` — последнее visual выделение.

**Тест:** `ma` → navigate → `'a` → вернулся на позицию.

---

## Шаг 11. Registers

**Файл:** `omniscope-tui/src/input/registers.rs`
**Референс:** `VIM_MOTIONS §2.7`

```rust
pub struct RegisterMap {
    named: HashMap<char, RegisterValue>,  // "a-"z
    numbered: VecDeque<RegisterValue>,    // "0-"9
    clipboard: Option<RegisterValue>,     // "+
    black_hole: (),                       // "_
}
```

- `"0` — последний yank; `"1-"9` — история удалений (FIFO)
- `"+` — системный буфер обмена (через arboard или xclip)
- `"_` — black hole (ничего не сохраняет)
- `"ayy` — yank в регистр `a`; `"ap` — paste из регистра `a`
- `:reg` — показать все регистры
- `:reg {a}` — показать конкретный

---

## Шаг 12. VISUAL mode

**Файл:** `omniscope-tui/src/input/visual.rs`
**Референс:** `VIM_MOTIONS §4`

Три вида Visual:
- `v` — VISUAL: отдельные книги, toggle по `Space`
- `V` — VISUAL-LINE: диапазон строк (anchor + cursor)
- `Ctrl+v` — VISUAL-BLOCK: выделение по колонкам (поля карточек)

В Visual mode доступны все операторы из Normal mode.
`gv` — восстановить последнее visual выделение.
`o` — переключить конец выделения (как в vim).
`Ctrl+a` — выделить всё.

Индикатор в статус-баре: `"5 selected"` при выделении 5 книг.

---

## Шаг 13. INSERT mode — форма редактирования

**Файл:** `omniscope-tui/src/input/insert.rs`
**Референс:** `VIM_MOTIONS §3`

INSERT mode открывает форму редактирования карточки.
- `Tab/Shift+Tab` — следующее/предыдущее поле
- `Ctrl+j/k` — следующее/предыдущее поле (альтернатива)
- `Ctrl+w` — удалить слово назад; `Ctrl+u` — очистить поле; `Ctrl+k` — удалить до конца
- `Ctrl+n/p` — автодополнение из существующих тегов/авторов
- `Esc` — сохранить и выйти в Normal; второй `Esc` — отменить изменения

Быстрые изменения без полного открытия формы (из Normal mode):
`cT` title, `ca` author, `cy` year, `ct` tags, `cs` status, `cr` rating, `cn` notes.
Открывают inline-редактор только для одного поля.

---

## Шаг 14. COMMAND mode

**Файл:** `omniscope-tui/src/input/command.rs`
**Референс:** `VIM_MOTIONS §8`

Базовые команды (приоритет):
```
:q :quit :wq :qa :q!
:w :write
:e {path}              — открыть файл/карточку
:lib {name}            — переключиться на библиотеку
:tag {name}            — фильтр по тегу
:sort {field}          — сортировка
:tabnew                — новая вкладка
:bnext/:bprev          — следующий/предыдущий буфер
:marks                 — показать метки
:reg                   — показать регистры
:undolist              — история отмен
:doctor                — диагностика
```

Глобальные команды:
```
:g/{pattern}/{command}        — применить команду к совпадающим
:g/status:unread/cs reading   — все непрочитанные → в читаемые
:g/#rust/ml programming       — все книги с тегом rust → в библиотеку
```

История команд: `Up/Down` для навигации. Автодополнение по `Tab`.

---

## Шаг 15. SEARCH mode — интеграция с Telescope

**Файл:** `omniscope-tui/src/input/search.rs`
**Референс:** `VIM_MOTIONS §6`

`/` — открыть fuzzy поиск по заголовку/автору/тегам (nucleo).
`?` — поиск в обратном направлении (в рамках текущего отображения).
`n/N` — следующее/предыдущее совпадение.
`*` — поиск слова под курсором (автор текущей книги).
`#` — то же, обратно.

Расширенные фильтры:
```
@author:Knuth          — поиск по автору
@year:>2020            — по году
@status:unread         — по статусу
@tag:rust              — по тегу
@lang:en               — по языку
```

`Ctrl+q` — отправить результаты поиска в quickfix list.

**Бенчмарк:** fuzzy поиск по 1000 книг < 10ms, по 10 000 книг < 30ms.

---

## Шаг 16. Undo/Redo система

**Файл:** `omniscope-core/src/undo.rs`
**Референс:** `VIM_MOTIONS §5.5`

Каждое мутирующее действие сохраняет `snapshot_before: BookCard` (или список карточек).
`UndoStack` — вектор `UndoEntry { action_desc, snapshot, timestamp }`.

- `u` — undo (восстановить последний snapshot)
- `Ctrl+r` — redo
- `U` — undo всех изменений текущей карточки (с момента открытия)
- `:undolist` — показать историю
- `:earlier 5m` — откатиться на 5 минут назад
- `:later 1h` — вперёд на 1 час

**Важно:** все AI-действия тоже должны быть undoable — это инвариант проекта.

---

## Шаг 17. Macros

**Файл:** `omniscope-tui/src/input/macro_recorder.rs`
**Референс:** `VIM_MOTIONS §9`

- `q{a-z}` — начать запись макроса в регистр `a`
- `q` — остановить запись
- `@{a-z}` — воспроизвести макрос
- `@@` — повторить последний макрос
- `[N]@{a}` — повторить N раз
- `:macros` — список записанных макросов

`MacroRecorder` — пишет сырые KeyEvents в буфер во время записи.
При воспроизведении — заново подаёт события в input pipeline.

**Тест:** записать макрос `qa cT [new title] Esc j q`, выполнить `5@a` → 5 книг с новым заголовком.

---

## Шаг 18. Quickfix List

**Файл:** `omniscope-tui/src/input/quickfix.rs`
**Референс:** `VIM_MOTIONS §10`

Quickfix — временный список книг для пакетных операций.
- `Ctrl+q` — отправить результаты поиска/visual в quickfix
- `:copen` — открыть quickfix панель
- `:cclose` — закрыть
- `cn/cp` — следующая/предыдущая запись в quickfix
- `:cdo {command}` — применить команду к каждой книге в quickfix

Использование: поиск → `Ctrl+q` → `:cdo @t` (AI: предложить теги каждой книге).

---

## Шаг 19. f/F/t/T — прыжки по первой букве

**Файл:** `omniscope-tui/src/input/find_char.rs`
**Референс:** `VIM_MOTIONS §11.2`

- `f{char}` — прыгнуть к ближайшей книге, заголовок которой начинается с `{char}`
- `F{char}` — то же, в обратном направлении
- `t{char}` — прыгнуть к книге перед той, что начинается с `{char}`
- `T{char}` — то же, в обратном направлении
- `;` — повторить последний f/F/t/T вперёд
- `,` — повторить в обратном направлении

---

## Шаг 20. EasyMotion

**Файл:** `omniscope-tui/src/input/easy_motion.rs`
**Референс:** `VIM_MOTIONS §11.3`

`Space Space` — overlay с метками `[a][b][c]...` на каждой видимой книге.
Нажать метку → прыгнуть к этой книге.

`Space j/k` — EasyMotion только ниже/выше курсора.
`Space /` — EasyMotion по первой букве заголовка.

Реализация: рендерить overlay поверх списка, перехватывать один символ, прыгать.

---

## Шаг 21. Keybindings — конфигурация

**Файл:** `omniscope-core/src/config/keybindings.rs`
**Референс:** `VIM_MOTIONS §12`

Загружать `~/.config/omniscope/keybindings.toml` при старте.
Структура: `[normal]`, `[insert]`, `[visual]`, `[leader]`, `[normal.go]`.

```toml
[normal]
"<C-j>" = "next-panel"
"<leader>t" = "tag-picker"

[normal.go]
"gl" = "goto-last-library"
```

Парсить строки типа `"<C-j>"`, `"<leader>t"`, `"<S-Enter>"` в `KeyEvent`.
Переопределения применяются поверх дефолтных биндингов.
`"noop"` — отключить клавишу.

---

## Шаг 22. Интеграционные тесты и бенчмарки

**Файл:** `omniscope-tui/tests/vim_integration.rs`

Покрыть все 5 workflow из `VIM_MOTIONS §15`:

1. Добавить и организовать книгу: `A → i → Esc → @m → ml`
2. Массовая реорганизация: `/ → Ctrl+q → :copen → cn → @t`
3. Перенести все книги автора: `/* → Ctrl+q → :g/@author:.../ml fiction`
4. Ежедневный ревью: `:tabnew tag:unread → gg → j/k → cs → @a`
5. Макрос для нормализации: `qa → cT → Esc → @m → j → q → 20@a`

**Бенчмарки (CI):**
- `j` навигация на 10 000 книг: < 5ms
- fuzzy поиск 1 000 книг: < 10ms
- fuzzy поиск 10 000 книг: < 30ms
- full-text (tantivy): < 50ms
- рендеринг кадра TUI: < 16ms

---

## Шаг 23. Финальная полировка

- Cheatsheet `:help` / `?` — интерактивный оверлей из `VIM_MOTIONS §14`
- PENDING indicator: подсказки при ожидании второй клавиши
- Все `leader`-команды в `[leader]` секции keybindings.toml
- Smoke test: пройти вручную все 5 workflow сценария

---

## Порядок зависимостей (граф)

```
Шаги 0→1→2 — обязательный фундамент
Шаг 3 — зависит от 0,1,2
Шаг 4 — зависит от 3
Шаг 5 — зависит от 3,4
Шаг 6 — зависит от 1,2
Шаг 7 — зависит от 6
Шаги 8,9 — зависят от 6
Шаги 10,11 — зависят от 3,6
Шаг 12 — зависит от 6,10
Шаг 13 — зависит от 1
Шаг 14 — зависит от 1
Шаг 15 — зависит от 3
Шаг 16 — зависит от 6,7 (все мутации должны быть готовы)
Шаг 17 — зависит от всего pipeline (шаги 1-15)
Шаги 18,19,20 — зависят от 3,15
Шаг 21 — зависит от 1,6
Шаг 22 — зависит от всего
Шаг 23 — последний
```

---

*Vim — это язык. Реализуй грамматику, а не список хоткеев.*
