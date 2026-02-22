# 🔬 Пошаговый план реализации научного модуля Omniscope

> **Крейт:** `omniscope-science`
> **Спек:** `Omniscope_SCIENCE.md` — читать указанный раздел ПЕРЕД каждым шагом
> **Правило агента:** после каждого шага — `cargo test`, `cargo clippy --deny warnings`. Красные тесты = стоп.
> **Коммит:** каждый шаг — один атомарный коммит с осмысленным сообщением

---

## Принципы, которые нельзя нарушать

```
1. unwrap()/expect() — только в тестах и в main(), никогда в библиотечном коде
2. Все HTTP-запросы — только через RateLimitedClient, не голый reqwest
3. Все внешние ответы — кэшировать на диск через DiskCache
4. Тест пишется ДО реализации для всех парсеров и валидаторов (TDD)
5. Ни один источник (Anna's, Sci-Hub) не должен блокировать приложение при недоступности
6. Приоритет источников при слиянии: UserManual(100) > CrossRef(90) > arXiv(85) > PDF(80) > S2(65) > AI(40)
```

---

## Шаг 0. Scaffolding крейта

**Референс:** `SCIENCE.md §0` (философия), `MASTER_PLAN.md §0.1` (структура проекта)

**Что сделать:**

Создать `omniscope-science/Cargo.toml` с зависимостями:
- `quick-xml` — парсинг Atom XML от arXiv
- `scraper` — HTML парсинг без официального API (Anna's Archive, Sci-Hub)
- `regex` + `once_cell` — ленивая компиляция паттернов
- `reqwest` с features `["json", "gzip", "stream"]`
- `mockito` в `dev-dependencies` — моки HTTP в тестах
- Все остальное из workspace (`serde`, `tokio`, `thiserror`, `anyhow`, `chrono`, `tracing`)

Создать пустые файлы-заглушки (только `todo!()`) для всех модулей:

```
omniscope-science/src/
├── lib.rs
├── error.rs
├── http.rs                   RateLimitedClient + DiskCache
├── config.rs                 ScienceConfig (из SCIENCE.md §10)
├── identifiers/
│   ├── mod.rs
│   ├── doi.rs
│   ├── arxiv.rs
│   ├── isbn.rs
│   └── extract.rs            regex-поиск в произвольном тексте
├── types.rs                  ScientificIdentifiers, CitationGraph, OpenAccessInfo, DocumentType
├── arxiv/
│   ├── mod.rs
│   ├── client.rs             ArxivClient
│   ├── types.rs              ArxivMetadata, ArxivSearchQuery
│   └── parser.rs             Atom XML → ArxivMetadata
├── sources/
│   ├── mod.rs                ExternalSource trait
│   ├── crossref.rs
│   ├── semantic_scholar.rs
│   ├── openalex.rs
│   ├── unpaywall.rs
│   ├── openlibrary.rs
│   ├── core_ac.rs
│   ├── annas_archive.rs
│   └── scihub.rs
├── references/
│   ├── mod.rs
│   ├── extractor.rs          полный пайплайн PDF → ссылки
│   ├── parser.rs             find_references_section, parse_reference_lines
│   └── resolver.rs           разрешение через CrossRef/S2
├── enrichment/
│   ├── mod.rs
│   ├── pipeline.rs           enrich_metadata_pipeline
│   └── merge.rs              source_priority, MergeStrategy
├── formats/
│   ├── mod.rs
│   ├── bibtex.rs
│   ├── ris.rs
│   └── csl.rs
└── dedup.rs                  дедупликация по идентификаторам
```

Добавить крейт в корневой `Cargo.toml` workspace.

**Проверка:** `cargo build --package omniscope-science` без ошибок компиляции.

---

## Шаг 1. ScienceError

**Референс:** `SCIENCE.md §3.1` (SourceStatus), общие принципы обработки ошибок

**Что сделать:**

Реализовать `ScienceError` в `error.rs` с вариантами:
- `InvalidDoi(String)` — невалидный DOI
- `InvalidArxivId(String)` — невалидный arXiv ID
- `InvalidIsbn(String)` — невалидный ISBN / неверная контрольная цифра
- `Http(#[from] reqwest::Error)` — сетевая ошибка
- `ApiError { source: String, message: String }` — ошибка от API (4xx/5xx)
- `RateLimit { source: String, retry_after_secs: u64 }` — превышен лимит
- `NoMirror { source: String }` — ни одно зеркало не доступно
- `Parse(String)` — ошибка парсинга XML/JSON/HTML
- `PdfExtraction(String)` — pdftotext недоступен или файл повреждён
- `SourceUnavailable(String)` — источник полностью недоступен
- `Cache(String)` — ошибка дискового кэша

**Проверка:** `cargo test -p omniscope-science` — компилируется, тесты пустые но зелёные.

---

## Шаг 2. Типизированные идентификаторы

**Референс:** `SCIENCE.md §1.1` — полное описание `Doi`, `ArxivId`, `Isbn`

Три подшага, каждый с TDD.

### Шаг 2а. Doi

**Файл:** `identifiers/doi.rs`

Реализовать `Doi { raw, normalized, url }` с методом `Doi::parse(input: &str) -> Result<Doi, ScienceError>`.

Парсер должен принимать все форматы:
- `10.1000/xyz123` — голый DOI
- `doi:10.1000/xyz123` — с префиксом
- `DOI: 10.1000/xyz123` — с пробелом
- `https://doi.org/10.1000/xyz123` — полный URL
- `http://dx.doi.org/10.1000/xyz123` — старый формат

Валидация: должно начинаться с `10.`, содержать `/`, после суффикса непустая строка.
`normalized` — всегда lowercase без префикса.

Написать тесты ДО реализации:
- корректный голый DOI
- DOI с https-префиксом
- DOI с `doi:` префиксом
- DOI с пробелом после двоеточия
- отклонить `not-a-doi`
- отклонить `10.1000` без суффикса
- отклонить пустую строку

### Шаг 2б. ArxivId

**Файл:** `identifiers/arxiv.rs`

Реализовать `ArxivId { raw, id, version, abs_url, pdf_url, category }` с методом `ArxivId::parse`.

Принимать форматы (все из `SCIENCE.md §2.1`):
- `2301.04567` — новый формат
- `2301.04567v2` — с версией
- `cs.AI/0601001` — старый формат с категорией
- `arxiv:2301.04567` — с префиксом
- `arXiv:2301.04567v5`
- `https://arxiv.org/abs/2301.04567`
- `https://arxiv.org/pdf/2301.04567`

`id` — всегда без версии. `version` — `Some(u8)` если указана.

Написать тесты: все форматы выше + отклонить `12345`, `not-arxiv`, `123.456` (слишком короткий).

### Шаг 2в. Isbn

**Файл:** `identifiers/isbn.rs`

Реализовать `Isbn { raw, isbn13, isbn10, formatted }` с `Isbn::parse`.

Логика:
- Убрать дефисы и пробелы, определить длину (10 или 13 цифр + X)
- Проверить контрольную цифру (ISBN-10: mod 11; ISBN-13: mod 10)
- Конвертировать ISBN-10 → ISBN-13 (добавить префикс `978`, пересчитать чек)
- `isbn10` — `None` для ISBN-13 с префиксом `979`

Написать тесты: валидный ISBN-13, ISBN-13 с дефисами, ISBN-10, ISBN-10 с `X`, неверная контрольная цифра → ошибка.

**Проверка:** все тесты зелёные. `cargo clippy --deny warnings` без замечаний.

---

## Шаг 3. ScientificIdentifiers и базовые типы

**Референс:** `SCIENCE.md §1.1, §1.2, §7.1`

**Файл:** `types.rs`

Реализовать структуры данных без логики — только типы:

`ScientificIdentifiers` — полная структура из `§1.1`: все 25+ полей (`isbn: Vec<Isbn>`, `doi: Option<Doi>`, `arxiv_id: Option<ArxivId>`, `pmid`, `pmcid`, `s2_paper_id`, `openalex_id`, `mag_id`, `dblp_key`, `openlibrary_id`, и т.д.). Все поля с `#[serde(default, skip_serializing_if = "Option::is_none")]`.

`CitationGraph { citation_count, reference_count, influential_citation_count, last_updated, references: Vec<String>, cited_by_sample: Vec<String> }`.

`OpenAccessInfo { is_open, status, license, oa_url, pdf_urls: Vec<String> }`.

`DocumentType` — полная таксономия из `§7.1`: `Book`, `BookChapter`, `Textbook`, `JournalArticle`, `ReviewArticle`, `ConferencePaper`, `Preprint`, `WorkingPaper`, `TechnicalReport`, `PhdThesis`, `MasterThesis`, `Standard`, `Patent`, `Dataset`, `Software`, `Other(String)`, и остальные варианты.

Методы на `DocumentType`:
- `from_crossref_type(s: &str) -> Self` — маппинг из CrossRef type strings
- `to_bibtex_type(&self) -> &'static str` — в BibTeX entry type

`ScienceConfig` в `config.rs` — все поля из `SCIENCE.md §10`: `polite_pool_email`, `semantic_scholar_api_key`, `core_api_key`, `auto_extract_doi_from_pdf`, `preferred_pdf_sources`, `rename_scheme`, секции `scihub`, `annas_archive`, `export`, `citation_graph`.

**Проверка:** сериализация/десериализация roundtrip в тесте (serde_json).

---

## Шаг 4. HTTP-инфраструктура

**Референс:** `SCIENCE.md §3.1` — rate_limit в ExternalSource trait

**Файл:** `http.rs`

Это фундамент. Все источники строятся поверх этих двух компонентов.

### RateLimitedClient

Обёртка над `reqwest::Client` с полями:
- `min_interval: Duration` — минимальная пауза между запросами
- `last_request: Arc<Mutex<Option<Instant>>>` — время последнего запроса
- `max_retries: u32`

Методы:
- `get(url) -> Result<String>` — GET с соблюдением интервала
- `get_with_headers(url, headers) -> Result<String>` — GET с кастомными заголовками
- `get_json<T: DeserializeOwned>(url) -> Result<T>` — GET + десериализация
- `post_json<T: Serialize, R: DeserializeOwned>(url, body) -> Result<R>` — POST

Логика retry: при `429 Too Many Requests` — читать заголовок `Retry-After`, ждать, повторить до `max_retries`. При сетевой ошибке — экспоненциальная задержка (`2^attempt` секунды).

### DiskCache

Структура `DiskCache { dir: PathBuf, ttl: Duration }`.

Конструктор `DiskCache::new(namespace, ttl)` кладёт кэш в `~/.local/share/omniscope/cache/{namespace}/`.

Ключ → файл по хешу ключа: `{hash:016x}.json`.

Методы:
- `async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>` — `None` если нет файла или TTL истёк
- `async fn set<T: Serialize>(&self, key: &str, value: &T)` — ошибки игнорировать (кэш не критичен)
- `async fn invalidate(&self, key: &str)` — удалить запись

Написать тесты: `set` → `get` возвращает то же значение; после TTL `get` возвращает `None`.

**Проверка:** тесты зелёные.

---

## Шаг 5. ExternalSource trait

**Референс:** `SCIENCE.md §3.1` — полная спецификация trait

**Файл:** `sources/mod.rs`

Объявить trait (не реализовывать — только определить):

```
trait ExternalSource: Send + Sync {
    fn name() -> &str
    fn source_type() -> SourceType
    fn requires_auth() -> bool
    fn rate_limit() -> RateLimit

    async fn search(query) -> Result<Vec<SearchResult>>
    async fn fetch_metadata(id) -> Result<Option<Metadata>>
    async fn find_download_url(id) -> Result<Option<DownloadUrl>>
    async fn health_check() -> SourceStatus
}
```

Вспомогательные типы: `SourceType` (enum: `AcademicMetadata`, `BookMetadata`, `Search`, `Download`, `OpenAccess`), `SourceStatus { available, latency_ms, last_checked, mirror }`, `RateLimit { requests_per_second: f32 }`.

Также объявить общие типы результатов поиска:
- `SearchResult { title, authors, year, identifier, source, relevance_score }`
- `DownloadUrl { url, source_name, requires_redirect: bool }`

**Проверка:** `cargo build` без ошибок.

---

## Шаг 6. ArXiv клиент

**Референс:** `SCIENCE.md §2.1, §2.2`

### Шаг 6а. Типы ArXiv

**Файл:** `arxiv/types.rs`

`ArxivMetadata { arxiv_id, doi, title, authors: Vec<ArxivAuthor>, abstract_text, published, updated, categories, primary_category, comment, journal_ref, pdf_url, abs_url }`.

`ArxivAuthor { name, affiliation }`.

`ArxivSearchQuery` со всеми полями из `§2.2`: `all`, `title`, `author`, `abstract_text`, `category`, `journal`, `id_list`, `sort_by`, `max_results`, `start`, `date_from`, `date_to`.

Метод `ArxivSearchQuery::to_query_string() -> String` — собирает строку запроса для API (`ti:attention+AND+au:Vaswani`). Написать тесты для сборки строки.

### Шаг 6б. Atom XML парсер

**Файл:** `arxiv/parser.rs`

Функция `parse_atom_response(xml: &str) -> Result<Vec<ArxivMetadata>>`.

Использовать `quick-xml` для десериализации Atom-фида. Ключевые поля из XML:
- `<id>` → извлечь arXiv ID (убрать `http://arxiv.org/abs/`)
- `<title>` → очистить переносы строк
- `<author>` + `<arxiv:affiliation>` → `Vec<ArxivAuthor>`
- `<summary>` → abstract, убрать переносы строк
- `<published>`, `<updated>` → парсинг RFC3339
- `<category term="cs.CL">` → список категорий
- `<arxiv:doi>` → `Option<Doi>` через `Doi::parse`
- `<link type="application/pdf">` → pdf_url

Обязательно написать тест с реальным XML-фикстуром статьи "Attention Is All You Need" — проверить все поля.

### Шаг 6в. ArxivClient

**Файл:** `arxiv/client.rs`

`ArxivClient` содержит `RateLimitedClient` (3 сек между запросами — официальная рекомендация arXiv) и `DiskCache` (TTL 7 дней).

Методы:
- `fetch_metadata(id: &ArxivId) -> Result<ArxivMetadata>` — GET `http://export.arxiv.org/api/query?id_list={id}`, парсинг через `parse_atom_response`
- `search(query: &ArxivSearchQuery) -> Result<Vec<ArxivMetadata>>` — GET с `search_query=...&max_results=...&sortBy=...`
- `check_for_updates(id: &ArxivId, current_version: Option<u8>) -> Result<Option<ArxivMetadata>>` — `Some` если версия на arXiv новее

Оба метода проверяют кэш перед запросом, сохраняют в кэш после успешного ответа.

Написать тест с mockito: мок HTTP → проверить что клиент корректно парсит ответ и возвращает метаданные.

**Проверка:** `cargo test -p omniscope-science arxiv` — все зелёные.

---

## Шаг 7. Regex-извлечение идентификаторов из текста

**Референс:** `SCIENCE.md §2.3` — `extract_doi_from_text`, `extract_arxiv_id_from_text`

**Файл:** `identifiers/extract.rs`

Все regex — через `once_cell::sync::Lazy`, компилируются один раз.

Реализовать функции:

`extract_dois_from_text(text: &str) -> Vec<Doi>` — находит все DOI по паттернам:
- голый `10.XXXX/...`
- `doi:10.XXXX/...`
- `https://doi.org/10.XXXX/...`
- `DOI: 10.XXXX/...`

`extract_arxiv_ids_from_text(text: &str) -> Vec<ArxivId>` — находит все arXiv ID по паттернам:
- `arXiv:2301.04567`
- `arxiv.org/abs/2301.04567`
- `[2301.04567]` в начале строки ссылки

`extract_isbn_from_text(text: &str) -> Option<Isbn>` — ищет ISBN-10 и ISBN-13.

`find_doi_in_first_page(pdf_path: &Path) -> Result<Doi>` — запускает `pdftotext -f 1 -l 2 {path} -`, затем `extract_dois_from_text`. Возвращает ошибку если pdftotext не найден или DOI не обнаружен.

`find_arxiv_id_in_pdf(pdf_path: &Path) -> Result<ArxivId>` — аналогично для arXiv ID.

Написать тесты (без реальных PDF): на строках с разными форматами идентификаторов. Проверить отсутствие ложных срабатываний (числа `10.5`, `2021.01` не должны матчиться).

**Проверка:** `cargo test -p omniscope-science identifiers` — все зелёные.

---

## Шаг 8. Источники метаданных

**Референс:** `SCIENCE.md §4.1–4.5`

Четыре источника по одному подшагу. Каждый реализует `ExternalSource` trait.

### Шаг 8а. CrossRef

**Файл:** `sources/crossref.rs`

`CrossRefSource { client: RateLimitedClient, cache: DiskCache, polite_email: Option<String> }`.

Rate limit: 100ms между запросами (с polite email — до 50 req/sec, без — ~3 req/sec, ставим 100ms как safe default).

User-Agent: `"omniscope/0.1 (mailto:{email})"` если email задан — это и есть polite pool.

Методы:
- `fetch_by_doi(doi: &Doi) -> Result<CrossRefWork>` — GET `https://api.crossref.org/works/{doi.normalized}`
- `query_by_text(reference: &str) -> Result<Option<(Doi, f32)>>` — Text Query API, возвращает `(doi, confidence_score)`. Порог уверенности: 80.0. Ниже — вернуть `None`.
- `fetch_batch(dois: &[Doi]) -> Result<Vec<CrossRefWork>>` — параллельные запросы через `buffer_unordered(5)`, не более 5 одновременно.

`CrossRefWork` содержит поля из `§4.1`: `doi, title: Vec<String>, author: Vec<CrossRefAuthor>, published_year, work_type: DocumentType, container_title, publisher, issn, isbn, abstract_text, reference_count, citation_count`.

`CrossRefWork::from_json(v: &Value) -> Result<Self>` — парсинг JSON-ответа.

Тест с mockito: проверить маппинг типов (`"journal-article"` → `JournalArticle`, `"book"` → `Book`).

### Шаг 8б. Semantic Scholar

**Файл:** `sources/semantic_scholar.rs`

`SemanticScholarSource { client, cache, api_key: Option<String> }`.

Rate limit: 1 сек без ключа (консервативно), 100ms с ключом. Если API-ключ есть — добавлять заголовок `x-api-key`.

Методы:
- `fetch_paper(id: &S2PaperId) -> Result<S2Paper>` — поддержать форматы: `"DOI:10.xxx"`, `"ArXiv:1706.xxx"`, `"{s2_hash}"`
- `fetch_batch(ids: &[S2PaperId]) -> Result<Vec<S2Paper>>` — POST на `/paper/batch`, до 500 ID
- `get_recommendations(paper_id: &str) -> Result<Vec<S2Paper>>` — recommendations API

`S2Paper` из `§4.2`: `paper_id, external_ids: HashMap<String, String>, title, abstract_text, year, authors, citation_count, reference_count, influential_citation_count, fields_of_study, is_open_access, open_access_pdf, tldr: Option<S2Tldr>`.

Тест: корректный парсинг `external_ids` — проверить что `DOI` и `ArXiv` ключи достаются.

### Шаг 8в. Unpaywall

**Файл:** `sources/unpaywall.rs`

`UnpaywallSource { client, cache, email: String }`.

Rate limit: 200ms. Кэш: 7 дней.

Метод `check_oa(doi: &Doi) -> Result<UnpaywallResult>` — GET `https://api.unpaywall.org/v2/{doi}?email={email}`.

`UnpaywallResult { doi, is_oa, oa_status, best_oa_location: Option<OaLocation>, oa_locations: Vec<OaLocation>, journal_is_oa }`.

`OaLocation { url, url_for_pdf, host_type, license, version }`.

Метод-хелпер `best_pdf_url(&self) -> Option<&str>` — возвращает лучший прямой PDF URL.

### Шаг 8г. OpenAlex

**Файл:** `sources/openalex.rs`

`OpenAlexSource { client, cache }`. Rate limit: 100ms (10 req/sec — их официальный лимит).

Методы:
- `fetch_work(id: &OpenAlexId) -> Result<OpenAlexWork>` — принимает `W...`, `doi:...`, `pmid:...`
- `search(query: &str, limit: u32) -> Result<Vec<OpenAlexWork>>`

`OpenAlexWork` из `§4.3`: `id, doi, title, publication_year, ids: OpenAlexIds, open_access: OpenAccessInfo, authorships, cited_by_count, referenced_works: Vec<String>`.

Метод `reconstruct_abstract(&self) -> Option<String>` — восстановить абстракт из `abstract_inverted_index` (инвертированный индекс OpenAlex: `{"word": [pos1, pos2], ...}`).

### Шаг 8д. Open Library (книги по ISBN)

**Файл:** `sources/openlibrary.rs`

`OpenLibrarySource { client, cache }`. Rate limit: 500ms.

Методы:
- `fetch_by_isbn(isbn: &Isbn) -> Result<OpenLibraryWork>` — GET `https://openlibrary.org/api/books?bibkeys=ISBN:{isbn13}&format=json&jscmd=data`
- `search_by_title(title: &str) -> Result<Vec<OpenLibraryWork>>`

`OpenLibraryWork { title, authors, publishers, publish_date, subjects, cover_url, openlibrary_id }`.

### Шаг 8е. CORE

**Файл:** `sources/core_ac.rs`

`CoreSource { client, cache, api_key: String }`.

Методы:
- `search(query: &str) -> Result<Vec<CoreWork>>` — только если `api_key` не пустой
- `fetch_by_doi(doi: &Doi) -> Result<Option<CoreWork>>`

`CoreWork { id, title, authors, abstract_text, doi, download_url: Option<String>, year }`.

Если API-ключ не задан — возвращать `Ok(vec![])` без запроса, логировать warning.

**Проверка:** `cargo test -p omniscope-science sources` — все зелёные.

---

## Шаг 9. Anna's Archive и Sci-Hub

**Референс:** `SCIENCE.md §3.2, §3.3`

⚠ Эти источники используют HTML-скрейпинг. CSS-селекторы могут меняться. Тесты проверяют парсинг по сохранённым HTML-фикстурам, не живые запросы.

### Шаг 9а. Anna's Archive

**Файл:** `sources/annas_archive.rs`

`AnnasArchiveSource { client: RateLimitedClient, active_mirror: Arc<RwLock<String>>, cache: DiskCache }`.

Зеркала из `§3.2`: `annas-archive.org`, `annas-archive.se`, `annas-archive.li`, `annas-archive.gs`.

Rate limit: 2 сек (1 запрос в 2 секунды — уважительно).

Методы:
- `search(query: &AnnasQuery) -> Result<Vec<AnnasResult>>` — HTML-скрейпинг страницы `/search?q=...&ext=...&lang=...`
- `get_download_links(md5: &str) -> Result<Vec<DownloadLink>>` — страница `/md5/{md5}`

`parse_search_html(html: &str) -> Result<Vec<AnnasResult>>` — через `scraper` (CSS-селекторы для карточек результатов). Извлечь: название, авторы, год, формат (pdf/epub/...), размер в МБ, язык, MD5-хэш из URL.

`fetch_with_mirror_rotation(url) -> Result<String>` — при ошибке пробовать следующее зеркало из списка; сохранять рабочее зеркало в `active_mirror`.

`AnnasQuery { q, ext: Vec<String>, lang, content }`.

`AnnasResult { title, authors, year, format, size_mb, language, md5, isbn, publisher }`.

`DownloadLink { source, url, priority: u8 }`.

Вспомогательные парсеры (на regex): `parse_year_from_meta`, `parse_format_from_meta`, `parse_size_from_meta`, `parse_lang_from_meta`.

Написать тест с HTML-фикстурой (сохранённой страницей поиска) — проверить что парсер возвращает список результатов с корректными полями.

### Шаг 9б. Sci-Hub

**Файл:** `sources/scihub.rs`

`SciHubSource { client: RateLimitedClient, working_mirror: Arc<RwLock<Option<String>>> }`.

Зеркала из `§3.3`: `sci-hub.se`, `sci-hub.st`, `sci-hub.ru`, `sci-hub.ren`, `sci-hub.mksa.top`.

Методы:
- `init() -> Result<()>` — найти рабочее зеркало при старте, сохранить в `working_mirror`. Если ни одно не доступно — логировать warning, не возвращать ошибку
- `fetch_by_doi(doi: &Doi) -> Result<SciHubResult>` — GET `{mirror}/{doi.normalized}`, HTML-парсинг
- `download_pdf(doi, output_dir, filename) -> Result<PathBuf>` — стриминговое скачивание

`parse_scihub_page(html: &str) -> Result<SciHubResult>` — найти `<iframe id="pdf" src="...">` или `<embed type="application/pdf" src="...">`. Нормализовать URL: `//sci-hub.se/...` → `https://sci-hub.se/...`.

`SciHubResult { pdf_url: Option<String>, title: Option<String> }`.

Написать тест с HTML-фикстурой страницы Sci-Hub.

**Проверка:** `cargo test -p omniscope-science` — все зелёные.

---

## Шаг 10. Парсер секции References из PDF

**Референс:** `SCIENCE.md §2.3`

**Файлы:** `references/parser.rs`, `references/resolver.rs`

### Шаг 10а. Нахождение секции References

**Файл:** `references/parser.rs`

`find_references_section(text: &str) -> Option<&str>` — найти в тексте секцию References по regex-паттерну заголовков (`References`, `Bibliography`, `Works Cited`, `Литература`, `Список литературы`). Конец секции = следующий крупный заголовок (`Appendix`, `Supplementary`, `Acknowledgements`) или конец файла.

`parse_reference_lines(section: &str) -> Vec<String>` — разбить на отдельные ссылки. Две стратегии: нумерованный список (`[1]`, `1.`, `1)`) и ненумерованный (разделение по пустым строкам). Вернуть только строки длиннее 20 символов.

Тесты: нумерованный список → правильное количество ссылок; ненумерованный (через `\n\n`) → разбивается корректно; заголовок `Appendix` после ссылок → не попадает в результат.

### Шаг 10б. Разрешение ссылок

**Файл:** `references/resolver.rs`

`resolve_unidentified(refs: &mut Vec<ExtractedReference>, crossref: &CrossRefSource)` — для каждой ссылки без DOI/arXiv вызвать `crossref.query_by_text()`. Запросы параллельно через `buffer_unordered(3)`. Установить `confidence` и `resolution_method = CrossRefQuery`.

`ExtractedReference { index, raw_text, doi, arxiv_id, isbn, resolved_title, resolved_authors, resolved_year, confidence: f32, resolution_method, is_in_library: Option<BookId> }`.

`ResolutionMethod` enum: `DirectDoi`, `DirectArxiv`, `CrossRefQuery`, `SemanticScholar`, `Unresolved`.

---

## Шаг 11. Полный пайплайн извлечения ссылок

**Референс:** `SCIENCE.md §2.3`

**Файл:** `references/extractor.rs`

`ReferenceExtractor { crossref: Arc<CrossRefSource>, s2: Arc<SemanticScholarSource> }`.

Метод `extract(card: &BookCard) -> Result<Vec<ExtractedReference>>`:

1. Если у книги есть `s2_paper_id` или `doi` или `arxiv_id` — попробовать получить ссылки из Semantic Scholar API (быстро, надёжно). Если успех — вернуть.
2. Если есть PDF-файл — `pdftotext {path} -` → `find_references_section` → `parse_reference_lines` → для каждой строки `extract_doi_from_text` + `extract_arxiv_id_from_text` → `resolve_unidentified` через CrossRef.
3. Если PDF нет и S2 не дал результат — вернуть `Ok(vec![])`.

После получения ссылок — для каждой с DOI или arXiv ID проверить, есть ли эта работа уже в библиотеке (через `db.find_by_doi` / `db.find_by_arxiv`), заполнить `is_in_library`.

**Проверка:** юнит-тест с моком S2 API и тест-PDF с реальными ссылками.

---

## Шаг 12. Пайплайн обогащения метаданных

**Референс:** `SCIENCE.md §6.1, §6.2`

**Файлы:** `enrichment/merge.rs`, `enrichment/pipeline.rs`

### Шаг 12а. Логика слияния

**Файл:** `enrichment/merge.rs`

`source_priority(source: MetadataSource) -> u8` — числовые приоритеты из `§6.2`: `UserManual=100`, `CrossRef=90`, `ArxivApi=85`, `PdfInternal=80`, `EpubOpf=75`, `OpenLibrary=70`, `SemanticScholar=65`, `OpenAlex=60`, `GoogleBooks=55`, `AiInferred=40`, `AnnasArchive=30`, `Unknown=10`.

`MergeStrategy` enum: `HighestPriority`, `Concat` (для авторов и тегов), `Longest` (для abstract), `UserOverride` (никогда не перезаписывать пользовательские данные).

Метод `BookCard::merge_metadata(new_data: PartialMetadata, source: MetadataSource)` — заполнять поле только если `source_priority(source) >= source_priority(existing_source)` для данного поля. Для авторов и тегов — объединять списки без дублей.

### Шаг 12б. Пайплайн

**Файл:** `enrichment/pipeline.rs`

`EnrichmentPipeline { crossref, s2, openalex, unpaywall, openlibrary, arxiv_client }`.

`EnrichmentReport { steps: Vec<String>, fields_updated: Vec<String>, sources_used: Vec<String>, errors: Vec<String> }`.

Метод `enrich(card: &mut BookCard) -> EnrichmentReport` — строго по порядку из `§6.1`:

**Этап 1 — Извлечение из файла:**
- PDF: `extract_pdf_metadata` (XMP + DocumentInfo через `lopdf` или `pdf-extract`) → `merge_metadata`. Если DOI ещё не задан — `find_doi_in_first_page`. Если arXiv ID не задан — `find_arxiv_id_in_pdf`.
- EPUB: парсинг OPF-файла внутри ZIP (Dublin Core метаданные).
- DjVu: пропустить на первой итерации (edge-case).

**Этап 2 — По идентификаторам:**
- `doi` → `crossref.fetch_by_doi` → `merge_metadata(Source::CrossRef)`
- `arxiv_id` → `arxiv_client.fetch_metadata` → `merge_metadata(Source::ArxivApi)`. Если у статьи есть DOI из arXiv и в карточке его нет — заполнить.
- `isbn` → `openlibrary.fetch_by_isbn` → `merge_metadata(Source::OpenLibrary)`

**Этап 3 — Semantic Scholar:**
- Определить S2 Paper ID: сначала `s2_paper_id`, иначе `DOI:{doi}`, иначе `ArXiv:{arxiv}`.
- Если нашли ID → `s2.fetch_paper` → обновить `citation_graph`, `tldr`, дополнить `external_ids` (pmid, dblp_key, mag_id).

**Этап 4 — Open Access:**
- Если есть DOI → `unpaywall.check_oa` → обновить `open_access`.

**Проверка:** интеграционный тест с реальным arXiv ID `1706.03762` — запросить обогащение, проверить что `citation_count > 0`, `open_access.is_open = true`.

---

## Шаг 13. Форматы экспорта и цитирования

**Референс:** `SCIENCE.md §5.1, §5.2, §5.3`

### Шаг 13а. BibTeX

**Файл:** `formats/bibtex.rs`

`generate_bibtex(card: &BookCard, opts: &BibTeXOptions) -> String` — генерирует BibTeX запись.

`BibTeXOptions { cite_key_scheme: CiteKeyScheme, utf8: bool }`.

`CiteKeyScheme` enum: `AuthorYear` (`"Vaswani2017"`), `AuthorYearTitle` (`"Vaswani2017Attention"`), `DoiBased`, `Custom(template: String)`.

`generate_cite_key(card, scheme) -> String` — логика из `§5.1`.

Сопоставление `DocumentType` → BibTeX entry type: `JournalArticle` → `@article`, `Book` → `@book`, `ConferencePaper` → `@inproceedings`, `Preprint` → `@misc`, `PhdThesis` → `@phdthesis`, остальные по таблице из `§5.1`.

Для статей с arXiv добавлять поля `arxivid`, `eprint`, `archivePrefix = {arXiv}`, `primaryClass`.

`parse_bibtex(content: &str) -> Result<Vec<BibEntry>>` — простой парсер BibTeX файла (handle `@type{key, field = {value}, ...}`). Можно использовать готовый крейт `nom-bibtex` или реализовать вручную.

Написать тест: `generate_bibtex` для "Attention Is All You Need" → проверить наличие `doi`, `arxivid`, `author` с несколькими авторами через `and`.

### Шаг 13б. RIS

**Файл:** `formats/ris.rs`

`generate_ris(card: &BookCard) -> String` — генерирует RIS-запись.

Маппинг: `JournalArticle` → `TY  - JOUR`, `Book` → `TY  - BOOK`, `ConferencePaper` → `TY  - CONF`, `Preprint` → `TY  - JOUR` (нет стандартного типа).

Обязательные поля: `TI` (title), `AU` (один тег на каждого автора), `PY` (год), `DO` (DOI), `UR` (URL), `ER  -` (конец записи).

`parse_ris(content: &str) -> Result<Vec<RisEntry>>` — парсинг `TY  - TYPE\nFIELD  - VALUE\nER  -` формата.

Тест: roundtrip `generate_ris` → `parse_ris` сохраняет DOI и заголовок.

### Шаг 13в. CSL-форматирование цитат

**Файл:** `formats/csl.rs`

Задача: форматировать цитату в стилях APA, IEEE, GOST без внешнего движка (он слишком тяжёлый для TUI).

Реализовать **встроенный минимальный процессор** для наиболее важных стилей:

`CslProcessor { locale: String }` с методом `format_citation(card: &BookCard, style: &str) -> Result<String>`.

Реализовать шаблоны для стилей из `§5.2`:
- `apa` — `Author, A. (Year). Title. Journal, Volume. doi:...`
- `ieee` — `A. Author et al., "Title," in Journal, year.`
- `gost-r-7-0-5-2008` — российский ГОСТ

`card_to_csl_item(card: &BookCard) -> CslItem` — промежуточный формат с полями `type, title, authors, year, journal, volume, doi, url, publisher`.

Написать тесты для каждого стиля — проверить формат вывода по известным примерам.

`format_bibliography(cards: &[&BookCard], style: &str) -> Result<Vec<String>>` — форматировать список.

**Проверка:** тесты для `bibtex::generate` и `csl::format_citation` — все зелёные.

---

## Шаг 14. Дедупликация

**Референс:** `SCIENCE.md §9` — команды `dedup --by-doi`, `--by-isbn`, `--by-title-fuzzy`

**Файл:** `dedup.rs`

`DuplicateFinder` с тремя стратегиями поиска дублей:

`find_by_doi(books: &[BookCard]) -> Vec<DuplicateGroup>` — сгруппировать книги с одинаковым `doi.normalized`. Разные версии arXiv (`v1` vs `v5`) — не дубли, если пользователь явно сохранил обе.

`find_by_isbn(books: &[BookCard]) -> Vec<DuplicateGroup>` — сгруппировать по `isbn13`. ISBN-10 и ISBN-13 одной книги — дублей не создают (это одна и та же книга).

`find_by_title_fuzzy(books: &[BookCard]) -> Vec<DuplicateGroup>` — нормализовать заголовки (lowercase, убрать пунктуацию, stemming не нужен), сравнить через Levenshtein или trigram similarity > 0.9. Использовать крейт `strsim`.

`DuplicateGroup { canonical: BookId, duplicates: Vec<BookId>, strategy: DedupStrategy }`.

`merge_duplicates(canonical: &BookId, to_merge: &[BookId], db: &Database) -> Result<()>` — оставить карточку с более полными метаданными, удалить остальные.

Тест: массив из 10 книг с 3 парами дублей → `find_by_doi` находит ровно 3 группы.

---

## Шаг 15. Добавление книги по arXiv ID / DOI

**Референс:** `SCIENCE.md §2.1` — полный flow `add_from_arxiv`

**Файл:** `arxiv/mod.rs` (или новый `add.rs`)

`ArxivAddOptions { download_pdf: bool, download_dir: Option<PathBuf>, auto_index: bool }`.

Функция `add_from_arxiv(id: &str, opts: ArxivAddOptions, db: &Database) -> Result<BookCard>`:
1. `ArxivId::parse(id)?`
2. Проверить дубликат в БД по arXiv ID → если нашли, вернуть существующую карточку
3. `arxiv_client.fetch_metadata(&arxiv_id)` → базовые метаданные
4. `s2.fetch_paper(S2PaperId::from_arxiv(&arxiv_id))` → citation_count, tldr (опционально, не блокировать если недоступно)
5. `unpaywall.check_oa(&doi)` → open_access (опционально)
6. Создать `BookCard` из собранных данных
7. Если `opts.download_pdf` → стриминг PDF с `arxiv_id.pdf_url`, сохранить в `opts.download_dir`, обновить `card.file`
8. Если `opts.auto_index` → вызвать AI индексацию (через `omniscope-ai`)
9. `db.insert_book_card(&card)` → сохранить
10. Вернуть карточку

`add_from_doi(doi: &str, opts: ..., db: &Database) -> Result<BookCard>` — аналогично через CrossRef, без скачивания (DOI не гарантирует PDF).

**Проверка:** интеграционный тест с реальным arXiv API (только если `CI_INTEGRATION=1`).

---

## Шаг 16. Проверка обновлений arXiv

**Референс:** `SCIENCE.md §9` — `arxiv update --all`

**Файл:** `arxiv/updater.rs`

`ArxivUpdater { client: Arc<ArxivClient>, db: Arc<Database> }`.

`check_all_updates() -> Result<Vec<ArxivUpdateResult>>` — выбрать все книги с `arxiv_id` из БД, для каждой вызвать `client.check_for_updates(id, current_version)`, собрать список тех у кого версия обновилась.

`ArxivUpdateResult { book_id, arxiv_id, old_version: Option<u8>, new_version: u8, new_metadata: ArxivMetadata }`.

`apply_updates(results: &[ArxivUpdateResult]) -> Result<()>` — обновить карточки в БД через `db.update_book_card`.

Этот метод вызывается проактивным нотификатором из `omniscope-ai` при старте приложения.

---

## Шаг 17. TUI — панель ссылок

**Референс:** `SCIENCE.md §2.4` — ASCII-макет панели References

**Файл:** `omniscope-tui/src/panels/references.rs`

`ReferencesPanel { references: Vec<ExtractedReference>, filter: RefsFilter, cursor: usize, scroll: usize }`.

`RefsFilter` enum: `All`, `Resolved`, `Unresolved`, `InLibrary`, `NotInLibrary`.

Отображение таблицы (из `§2.4`): колонки `#`, `Reference` (title + authors), `ID` (arXiv/DOI/ISBN), `In Library` (✓ / ✗).

Фильтр-бар в верхней части: `[all] [resolved] [unresolved] [in-library] [not-in-library]` — переключать Tab-ом.

Строки с `confidence < 0.7` — dim стиль (nord3), неразрешённые — добавить `[A]dd [F]ind` подсказки.

Клавиши в панели (из `§11`):
- `Enter` — открыть книгу если в библиотеке, иначе показать детали
- `a` — добавить ссылку в библиотеку (вызвать `add_from_doi`/`add_from_arxiv`)
- `f` — найти PDF онлайн (открыть Find & Download с этой ссылкой)
- `A` — добавить все неразрешённые
- `e` — экспортировать список в BibTeX/RIS
- `/` — поиск внутри списка

---

## Шаг 18. TUI — панель графа цитирований

**Референс:** `SCIENCE.md §2.5` — ASCII-макет Citation Graph

**Файл:** `omniscope-tui/src/panels/citation_graph.rs`

`CitationGraphPanel { book: BookCard, mode: GraphMode, references: Vec<CitationEdge>, cited_by: Vec<CitationEdge>, related: Vec<CitationEdge>, cursor: usize }`.

`GraphMode` enum: `References`, `CitedBy`, `Related`.

Рендер ASCII-дерева из `§2.5`:
- Корневой узел: `◉ {title} ({year})`
- Ветки `├──` и `└──`
- Индикаторы: `[✓]` если книга в библиотеке, `[✗]` если нет
- Суффикс с типом ID: `[arXiv]`, `[DOI]`, `[OpenAI]`

Переключение режима: `Tab` или `[References] [Cited By] [Related]` по номерам.

Клавиши: `Enter` — открыть книгу, `a` — добавить в библиотеку, `f` — найти PDF.

---

## Шаг 19. TUI — панель Find & Download

**Референс:** `SCIENCE.md §3.4` — ASCII-макет панели поиска

**Файл:** `omniscope-tui/src/panels/find_download.rs`

`FindDownloadPanel` — двухколоночный layout: слева Anna's Archive + Sci-Hub, справа Semantic Scholar + OpenAlex.

Строка поиска вверху + переключатели `[DOI] [arXiv] [ISBN] [PMID]`.

Индикаторы доступных источников: `[A✓] [S✓] [O✓] [G✓]` — показывать статус каждого источника (green = доступен, red = недоступен).

Для каждого результата: название, автор, год, формат/размер (для Anna's Archive), citation count (для S2), `[D]ownload [M]eta [↗]open`.

Индикатор "✓ In library" для результатов, которые уже есть в библиотеке.

Клавиши: `Tab` — переключить фокус между колонками, `D` — скачать, `M` — импортировать только метаданные, `↗` — открыть в браузере, `Esc` — закрыть.

---

## Шаг 20. TUI — карточка научной статьи

**Референс:** `SCIENCE.md §8.1` — ASCII-макет карточки

**Файл:** `omniscope-tui/src/panels/article_card.rs`

Расширить существующую правую панель preview для книг типа `JournalArticle` / `Preprint` / `ConferencePaper`.

Дополнительные секции по сравнению с обычной книгой (из `§8.1`):

`IDENTIFIERS` — показать DOI (с `[↗ open]`), arXiv ID (`[↗ abs] [↗ pdf]`), S2 ID, OpenAlex ID.

`METRICS` — `Citations: {count} (📈 +{delta} last month)`, `Influential: {count}`, `References: {count}`, `Fields: {fields_of_study}`.

`OPEN ACCESS` — статус (✓ Green OA / ✗ Closed), список PDF URL со звёздочкой у лучшего.

`TL;DR` — если есть `tldr` от Semantic Scholar, показать в отдельной секции.

Кнопки внизу: `[o]pen [r]eferences [c]itations [e]xport BibTeX [ai] [f]ind`.

---

## Шаг 21. Научные vim-команды

**Референс:** `SCIENCE.md §11` — полная карта клавиш

**Файл:** `omniscope-tui/src/input/science_bindings.rs`

Добавить в Normal mode (когда фокус на книге):

| Команда | Действие |
|---------|----------|
| `gr` | Открыть панель References |
| `gR` | Открыть панель Cited By |
| `gs` | Открыть панель похожих статей (S2 recommendations) |
| `gd` | Открыть DOI в браузере |
| `ga` | Открыть arXiv страницу в браузере |
| `gA` | Открыть arXiv PDF в браузере |
| `go` | Найти открытый PDF (Unpaywall → arXiv → CORE) |
| `yD` | Скопировать DOI в clipboard |
| `yA` | Скопировать arXiv ID в clipboard |
| `yB` | Скопировать BibTeX в clipboard |
| `yC` | Скопировать форматированную цитату (default style из конфига) |
| `cD` | Установить/исправить DOI (inline edit) |
| `cA` | Установить arXiv ID (inline edit) |
| `@e` | AI: обогатить метаданные |
| `@r` | AI: извлечь и разрешить ссылки |

Command mode:
- `:cite [style]` — показать цитату (default: `ieee`)
- `:bibtex` — показать BibTeX
- `:refs` — открыть панель ссылок
- `:cited-by` — открыть панель цитирований

---

## Шаг 22. CLI-команды научного модуля

**Референс:** `SCIENCE.md §9` — полный список команд

**Файл:** `omniscope-cli/src/commands/science.rs`

Реализовать все CLI-команды из `§9` с флагом `--json` для машиночитаемого вывода:

**arxiv:** `add {id} [--download-pdf] [--json]`, `search {query} [--author] [--category] [--year] [--max]`, `update {id}`, `update --all`

**doi:** `add {doi}`, `resolve {doi} [--json]`

**refs:** `extract {book-id} [--json]`, `list {book-id} [--filter]`, `add-all {book-id} [--download-pdfs]`, `graph {book-id}`, `cited-by {book-id}`, `related {book-id}`

**cite:** `cite {book-id} [--style] [--clipboard]`

**export:** `export bibtex {path|-}`, `export ris {path|-}` — с фильтрами `--library`, `--tag`

**fetch:** `fetch doi:{doi}`, `fetch pmid:{pmid}`, `fetch {url}`

**oa:** `oa check {book-id}`, `oa download {book-id}`

**enrich:** `enrich {book-id} [--sources]`, `enrich --all [--json]`

**ids:** `ids check {book-id}`, `ids set {book-id} [--doi] [--arxiv] [--isbn] [--pmid]`

**dedup:** `dedup [--by-doi] [--by-isbn] [--by-title-fuzzy] [--json]`

**stats:** `stats science`, `stats citations [--top N]`, `stats oa`

---

## Шаг 23. Статистика библиотеки

**Референс:** `SCIENCE.md §8.2` — ASCII-макет `LIBRARY STATISTICS`

**Файл:** `omniscope-science/src/stats.rs`

`LibraryStats { total: u32, by_type: HashMap<DocumentType, u32>, by_format: HashMap<String, u32>, by_year: BTreeMap<i32, u32>, citation_metrics: CitationMetrics, identifier_coverage: IdentifierCoverage }`.

`CitationMetrics { most_cited: Vec<(BookId, String, u32)>, total_citations: u64, average_citations: f64 }`.

`IdentifierCoverage { doi_pct: f32, arxiv_pct: f32, isbn_pct: f32, s2_pct: f32, open_access_pct: f32 }`.

Функция `compute_stats(db: &Database, library_id: Option<LibraryId>) -> Result<LibraryStats>`.

TUI-компонент `StatsPanel` — гистограммы по годам (ASCII bar chart: `▓▓▓▓▓ 12`), круговые диаграммы по типам (текстовые: `■ Articles: 23 (49%)`), прогресс-бары покрытия идентификаторов.

---

## Шаг 24. Конфигурация

**Референс:** `SCIENCE.md §10`

**Файл:** `omniscope-science/src/config.rs`

Реализовать `ScienceConfig` с полной десериализацией из TOML-файла `~/.config/omniscope/config.toml`.

Секции:
- `[science]` — `polite_pool_email`, ключи API, флаги автодействий, `preferred_pdf_sources: Vec<String>`, `download_directory`, `rename_scheme`
- `[science.scihub]` — `enabled`, `mirror_check_on_startup`, `preferred_mirrors`
- `[science.annas_archive]` — `enabled`, `preferred_formats`, `preferred_languages`
- `[science.export]` — `default_cite_style`, `cite_key_scheme`, `bibtex_utf8`
- `[science.citation_graph]` — `fetch_on_add`, `fetch_depth`, `max_citations_to_store`

`ScienceConfig::load() -> Result<Self>` — читать файл, fallback на `Default::default()`.

`rename_from_scheme(card: &BookCard, scheme: &str) -> String` — шаблонизатор для `{author}_{year}_{title_short}`.

---

## Шаг 25. Интеграция с omniscope-ai

**Референс:** `SCIENCE.md §9` — `@e`, `@r`, `@c` команды; `Omniscope_AI_SYSTEM.md §3.4` — действия AI

В `omniscope-ai/src/actions` добавить новые действия:

`ExtractReferences { book_id: BookId }` — запустить `ReferenceExtractor::extract`, результат сохранить в БД.

`EnrichMetadata { book_id: BookId, sources: Vec<String> }` — запустить `EnrichmentPipeline::enrich`, отчёт вернуть в TUI.

`CheckArxivUpdates` — запустить `ArxivUpdater::check_all_updates`, уведомить о новых версиях.

В `LibraryMap` добавить для каждой книги поля: `cit: Option<u32>` (citation_count), `oa: bool` (is_open_access), `ghost: bool` (нет файла на диске).

AI может отвечать: "У тебя 8 статей без DOI. Хочешь обогатить метаданные через CrossRef?"

---

## Финальная проверка: E2E сценарии

После завершения всех шагов выполнить вручную:

**Сценарий A — Добавить статью по arXiv:**
```
omniscope arxiv add 1706.03762 --download-pdf
→ Карточка создана, PDF скачан, citation_count > 0, open_access = true
```

**Сценарий B — Извлечь ссылки:**
```
omniscope refs extract {book-id}
→ Список ссылок, часть с DOI, часть неразрешённые, часть "In library"
```

**Сценарий C — Экспорт в BibTeX:**
```
omniscope export bibtex - --library ml-papers
→ Валидный BibTeX с doi, arxivid, eprint полями
```

**Сценарий D — Обогащение:**
```
omniscope enrich {book-id} --sources crossref,s2,unpaywall
→ Заполнены abstract, citation_count, open_access
```

**Сценарий E — Дедупликация:**
```
omniscope dedup --by-doi
→ Список пар дублей (если есть)
```

**Сценарий F — TUI:**
```
Открыть TUI → выбрать статью → gr → видим панель ссылок
gr → a → статья из списка ссылок добавлена в библиотеку
yB → BibTeX в clipboard
```

---

## Зависимости между шагами

```
0 → 1 → 2 → 3 → 4 → 5
                   ↓
            6 ← 4, 2, 1
            7 ← 2
            8 ← 4, 2, 3, 1
            9 ← 4, 1
           10 ← 7, 8а
           11 ← 10, 8б
           12 ← 8а, 8б, 8в, 6
           13 ← 3
           14 ← 2, 3
           15 ← 6, 8а, 8б, 8в
           16 ← 6, 15
           17 ← 11, TUI
           18 ← 8б, TUI
           19 ← 8д, 9а, 9б, TUI
           20 ← 8б, 8в, TUI
           21 ← 20, 17, 18, 19
           22 ← 6, 8, 11, 12, 13, 14, 15
           23 ← 12, 14
           24 ← везде используется
           25 ← 12, 16
```

---

*Научный модуль превращает Omniscope из менеджера файлов в исследовательскую среду. Строить снизу вверх: идентификаторы → HTTP → API-источники → пайплайны → TUI.*
