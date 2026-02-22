# 📚 omniscope — Научная составляющая: Полная спецификация

> Этот документ описывает всю академическую функциональность omniscope:
> идентификаторы, работу с arxiv, извлечение ссылок, внешние источники,
> интеграцию с научными базами данных и инструменты для исследователей.

---

## 0. Философия научного модуля

| Принцип | Описание |
|---|---|
| **"Идентификатор = ключ"** | DOI, arXiv ID, ISBN — первичные ключи для поиска и дедупликации |
| **"Граф ссылок"** | Каждая статья — узел в графе цитирований |
| **"Источники без API"** | Sci-Hub и Anna's Archive доступны через веб-скрейпинг |
| **"Offline-first"** | Все метаданные кэшируются локально, сеть нужна только для обогащения |
| **"Zotero-совместимость"** | Полная поддержка всех форматов Zotero (RDF, CSL, BibTeX) |

---

## 1. Система идентификаторов — полная поддержка

### 1.1 Все поддерживаемые идентификаторы

```rust
/// Научные идентификаторы — расширенная секция BookCard
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScientificIdentifiers {

    // ── КНИГИ ──────────────────────────────────────────────────────

    /// ISBN-10 или ISBN-13 (International Standard Book Number)
    /// Формат: 978-3-16-148410-0 или 0-306-40615-2
    pub isbn: Vec<Isbn>,

    /// ISSN (International Standard Serial Number) — для журналов/серий
    /// Формат: 2049-3630
    pub issn: Option<String>,

    // ── СТАТЬИ И ПРЕПРИНТЫ ────────────────────────────────────────

    /// DOI (Digital Object Identifier) — основной идентификатор статей
    /// Формат: 10.1000/xyz123 или https://doi.org/10.1000/xyz123
    pub doi: Option<Doi>,

    /// arXiv ID — идентификатор препринта
    /// Форматы: 2301.04567 | 2301.04567v2 | cs.AI/0601001 | abs/2301.04567
    pub arxiv_id: Option<ArxivId>,

    /// PubMed ID — NCBI/PubMed
    /// Формат: 12345678
    pub pmid: Option<u32>,

    /// PubMed Central ID
    /// Формат: PMC1234567
    pub pmcid: Option<String>,

    // ── АКАДЕМИЧЕСКИЕ БАЗЫ ────────────────────────────────────────

    /// Semantic Scholar Corpus ID
    pub s2_corpus_id: Option<String>,

    /// Semantic Scholar Paper ID (SHA)
    pub s2_paper_id: Option<String>,

    /// OpenAlex Work ID
    /// Формат: W2741809807
    pub openalex_id: Option<String>,

    /// MAG ID (Microsoft Academic Graph — legacy, но данные ещё используются)
    pub mag_id: Option<u64>,

    /// ACM Digital Library ID
    pub acm_id: Option<String>,

    /// IEEE Xplore Article Number
    pub ieee_id: Option<String>,

    /// DBLP key
    /// Формат: journals/corr/abs-2301-04567
    pub dblp_key: Option<String>,

    // ── КНИЖНЫЕ БАЗЫ ──────────────────────────────────────────────

    /// Open Library Work/Edition ID
    /// Формат: OL7353617M (Edition) | OL45883W (Work)
    pub openlibrary_id: Option<String>,

    /// Google Books Volume ID
    pub google_books_id: Option<String>,

    /// WorldCat OCLC Number
    pub oclc: Option<u64>,

    /// Goodreads ID (legacy)
    pub goodreads_id: Option<u64>,

    /// LibraryThing Work ID
    pub librarything_id: Option<u64>,

    // ── СПЕЦИАЛИЗИРОВАННЫЕ ────────────────────────────────────────

    /// zbMATH (математика)
    pub zbmath_id: Option<String>,

    /// MathSciNet MR Number (American Mathematical Society)
    pub mrnumber: Option<String>,

    /// JSTOR stable URL / ID
    pub jstor_id: Option<String>,

    /// ResearchGate Publication ID
    pub researchgate_id: Option<String>,

    /// CAS Registry Number (химия)
    pub cas_rn: Option<String>,

    /// EAN-13 (более широкий формат ISBN)
    pub ean: Option<String>,

    /// ASIN (Amazon Standard Identification Number) — для книг на Amazon
    pub asin: Option<String>,

    // ── РЕПОЗИТОРИИ ───────────────────────────────────────────────

    /// HAL ID (французский открытый архив)
    pub hal_id: Option<String>,

    /// SSRN (Social Science Research Network)
    pub ssrn_id: Option<String>,

    /// bioRxiv/medRxiv DOI (препринты биологии/медицины)
    pub biorxiv_doi: Option<String>,

    /// ChemRxiv ID
    pub chemrxiv_id: Option<String>,

    // ── ПАТЕНТЫ ───────────────────────────────────────────────────

    /// Patent number (USPTO/EPO/etc.)
    pub patent_number: Option<String>,

    /// Patent DOI
    pub patent_doi: Option<String>,
}

/// Типизированный DOI с валидацией
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Doi {
    pub raw: String,            // Как введён пользователем
    pub normalized: String,     // 10.1000/xyz123 (без https://doi.org/)
    pub url: String,            // https://doi.org/10.1000/xyz123
}

/// Типизированный arXiv ID
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArxivId {
    pub raw: String,            // Как введён: 2301.04567v2
    pub id: String,             // 2301.04567 (без версии)
    pub version: Option<u8>,    // 2 (версия препринта)
    pub abs_url: String,        // https://arxiv.org/abs/2301.04567
    pub pdf_url: String,        // https://arxiv.org/pdf/2301.04567
    pub category: Option<String>, // cs.AI, math.CO, etc.
}
```

### 1.2 Расширенная секция JSON-карточки для науки

```json
{
  "id": "uuid-v7",
  "metadata": {
    "title": "Attention Is All You Need",
    "subtitle": null,
    "authors": [
      {
        "name": "Vaswani, Ashish",
        "orcid": "0000-0001-5895-4784",
        "affiliation": "Google Brain",
        "email": null
      },
      { "name": "Shazeer, Noam", "orcid": null }
    ],
    "year": 2017,
    "month": 6,
    "day": 12,
    "abstract": "The dominant sequence transduction models...",
    "language": "en",
    "pages": 15,
    "volume": null,
    "issue": null,
    "edition": null
  },

  "identifiers": {
    "doi": {
      "raw": "10.48550/arXiv.1706.03762",
      "normalized": "10.48550/arXiv.1706.03762",
      "url": "https://doi.org/10.48550/arXiv.1706.03762"
    },
    "arxiv_id": {
      "raw": "1706.03762",
      "id": "1706.03762",
      "version": 5,
      "abs_url": "https://arxiv.org/abs/1706.03762",
      "pdf_url": "https://arxiv.org/pdf/1706.03762",
      "category": "cs.CL"
    },
    "pmid": null,
    "s2_paper_id": "204e3073870fae3d05bcbc2f6a8e263d9b72e776",
    "openalex_id": "W2963403868",
    "isbn": [],
    "issn": null,
    "mag_id": 2963403868
  },

  "publication": {
    "type": "preprint",
    "venue": "NeurIPS 2017",
    "journal": "Advances in Neural Information Processing Systems",
    "conference": "NeurIPS",
    "proceedings": "NeurIPS 2017",
    "publisher": "Curran Associates",
    "series": null,
    "chapter": null,
    "pages_range": "5998-6008"
  },

  "citation_graph": {
    "citation_count": 87654,
    "reference_count": 41,
    "influential_citation_count": 12450,
    "last_updated": "2025-01-15T00:00:00Z",
    "references": ["s2:abc123", "s2:def456"],    // ID статей-ссылок
    "cited_by_sample": ["s2:ghi789"]             // Небольшая выборка
  },

  "open_access": {
    "is_open": true,
    "status": "green",          // gold | green | bronze | closed
    "license": "arXiv non-exclusive license",
    "oa_url": "https://arxiv.org/pdf/1706.03762",
    "pdf_urls": [
      "https://arxiv.org/pdf/1706.03762",
      "https://proceedings.neurips.cc/paper/2017/file/..."
    ]
  },

  "file": { "...": "..." },
  "organization": { "...": "..." },
  "ai": { "...": "..." }
}
```

---

## 2. ArXiv интеграция — полная реализация

### 2.1 Добавление по arXiv ID

```
┌─────────────────────────────────────────────────────────────────────┐
│  ADD FROM ARXIV                                                      │
├─────────────────────────────────────────────────────────────────────┤
│  arXiv ID: 1706.03762_                                              │
│                                                                     │
│  Formats accepted:                                                  │
│  • 1706.03762          (new format)                                 │
│  • 1706.03762v5        (with version)                               │
│  • cs.AI/0601001       (old category format)                        │
│  • arxiv:1706.03762    (with prefix)                                │
│  • https://arxiv.org/abs/1706.03762  (full URL)                    │
│  • https://arxiv.org/pdf/1706.03762  (PDF URL)                     │
└─────────────────────────────────────────────────────────────────────┘
```

```rust
pub async fn add_from_arxiv(id: &str, opts: ArxivAddOptions) -> Result<BookCard> {
    let arxiv_id = ArxivId::parse(id)?;   // Валидация и нормализация

    // 1. Получить метаданные через официальный API
    let metadata = arxiv_api::fetch_metadata(&arxiv_id).await?;

    // 2. Попытаться получить доп. данные из Semantic Scholar
    let s2_data = semantic_scholar::fetch_by_arxiv(&arxiv_id).await.ok();

    // 3. Получить данные Open Access из Unpaywall
    let oa_data = unpaywall::fetch_by_doi(&metadata.doi).await.ok();

    // 4. Собрать карточку
    let mut card = BookCard::from_arxiv(metadata, s2_data, oa_data);

    // 5. Скачать PDF если нужно
    if opts.download_pdf {
        let pdf_path = download_arxiv_pdf(&arxiv_id, opts.download_dir).await?;
        card.file = Some(FileInfo::new(pdf_path));
    }

    // 6. AI-индексация если включена
    if opts.auto_index {
        ai::index_book(&mut card).await?;
    }

    Ok(card)
}
```

### 2.2 ArXiv API — детали

```rust
// Официальный API: http://export.arxiv.org/api/query
// Формат: Atom XML, без необходимости ключа

pub mod arxiv_api {
    const BASE_URL: &str = "http://export.arxiv.org/api/query";

    /// Получить метаданные по ID
    pub async fn fetch_metadata(id: &ArxivId) -> Result<ArxivMetadata> {
        let url = format!("{BASE_URL}?id_list={}", id.id);
        let xml = http_get(&url).await?;
        parse_atom_response(&xml)
    }

    /// Поиск по запросу
    pub async fn search(query: &ArxivSearchQuery) -> Result<Vec<ArxivMetadata>> {
        // Поддержка полей поиска:
        // ti: title, au: author, abs: abstract
        // co: comment, jr: journal, cat: category
        // rn: report number, id: arXiv ID, all: все поля
        let url = format!("{BASE_URL}?search_query={}&max_results={}",
                          query.to_string(), query.max_results);
        let xml = http_get(&url).await?;
        parse_search_response(&xml)
    }

    /// Поиск с полным DSL
    pub struct ArxivSearchQuery {
        pub all: Option<String>,          // all:electron
        pub title: Option<String>,        // ti:attention
        pub author: Option<String>,       // au:Vaswani
        pub abstract_text: Option<String>, // abs:transformer
        pub category: Option<String>,     // cat:cs.AI
        pub journal: Option<String>,      // jr:NeurIPS
        pub id_list: Vec<String>,         // Конкретные ID
        pub sort_by: ArxivSort,           // relevance | submittedDate | lastUpdatedDate
        pub sort_order: SortOrder,        // ascending | descending
        pub max_results: u32,             // До 2000 за раз
        pub start: u32,                   // Пагинация
        pub date_from: Option<NaiveDate>,
        pub date_to: Option<NaiveDate>,
    }
}
```

### 2.3 Извлечение ссылок из arXiv статей

```rust
/// Парсинг секции References из PDF/LaTeX
pub mod reference_extractor {

    /// Полный пайплайн извлечения ссылок
    pub async fn extract_references(card: &BookCard) -> Result<Vec<ExtractedReference>> {
        match &card.file {
            Some(file) if file.format == FileFormat::Pdf => {
                extract_from_pdf(&file.path).await
            }
            _ => {
                // Попытаться получить ссылки из Semantic Scholar или OpenAlex
                fetch_references_from_api(card).await
            }
        }
    }

    /// Извлечение из PDF через pdftotext + парсинг
    async fn extract_from_pdf(path: &Path) -> Result<Vec<ExtractedReference>> {
        // 1. Конвертировать PDF в текст через pdftotext (poppler)
        let text = pdftotext(path).await?;

        // 2. Найти секцию References
        let refs_section = find_references_section(&text)?;

        // 3. Парсинг каждой ссылки
        let raw_refs = parse_reference_lines(&refs_section);

        // 4. Идентификация: DOI, arXiv ID, ISBN
        let mut identified = Vec::new();
        for raw in raw_refs {
            let mut r = ExtractedReference::from_raw(raw);
            r.doi = extract_doi_from_text(&r.raw_text);
            r.arxiv_id = extract_arxiv_id_from_text(&r.raw_text);
            r.isbn = extract_isbn_from_text(&r.raw_text);
            identified.push(r);
        }

        // 5. Попытка разрешить неидентифицированные через CrossRef
        resolve_unidentified(&mut identified).await?;

        Ok(identified)
    }

    /// Регулярные выражения для идентификаторов в тексте
    fn extract_doi_from_text(text: &str) -> Option<Doi> {
        // DOI паттерны:
        // 10.XXXX/... (основной)
        // doi:10.XXXX/...
        // https://doi.org/10.XXXX/...
        // DOI: 10.XXXX/...
        let re = Regex::new(r"(?i)(?:doi:?\s*|https?://doi\.org/)?(10\.\d{4,}/\S+)").unwrap();
        re.captures(text)
            .and_then(|c| c.get(1))
            .map(|m| Doi::parse(m.as_str()).ok())
            .flatten()
    }

    fn extract_arxiv_id_from_text(text: &str) -> Option<ArxivId> {
        // arXiv паттерны:
        // arXiv:2301.04567 | arXiv:2301.04567v2
        // arxiv.org/abs/2301.04567
        // [2301.04567] в начале строки ссылки
        let re = Regex::new(
            r"(?i)(?:arxiv:?|arxiv\.org/abs/)(\d{4}\.\d{4,5}(?:v\d+)?|[a-z-]+/\d{7})"
        ).unwrap();
        re.captures(text)
            .and_then(|c| c.get(1))
            .map(|m| ArxivId::parse(m.as_str()).ok())
            .flatten()
    }

    /// Разрешить ссылку через CrossRef API (по тексту → DOI)
    async fn resolve_via_crossref(raw: &str) -> Result<Option<Doi>> {
        // CrossRef Query API — бесплатный, без ключа (но желателен email)
        let url = format!(
            "https://api.crossref.org/works?query={}&rows=1&select=DOI,title,author",
            urlenccode(raw)
        );
        // Ответ CrossRef + оценка similarity score
        let response = http_get_with_header(&url, "User-Agent", POLITE_POOL_EMAIL).await?;
        crossref::best_match_doi(&response, raw)
    }
}
```

### 2.4 TUI для просмотра ссылок

```
┌──────────────────────────────────────────────────────────────────────┐
│  REFERENCES — "Attention Is All You Need"         [41 references]   │
├──────────────────────────────────────────────────────────────────────┤
│  Filter: [all] [resolved] [unresolved] [in-library] [not-in-library]│
├────┬─────────────────────────────────────────┬───────┬──────────────┤
│ #  │ Reference                               │ ID    │ In Library   │
├────┼─────────────────────────────────────────┼───────┼──────────────┤
│  1 │ Bahdanau et al., 2014 — Neural Machine  │ arXiv │ ✓ In library │
│    │ Translation by Jointly Learning...      │       │              │
│  2 │ Cho et al., 2014 — Learning Phrase     │ arXiv │ ✗ Not found  │
│    │ Representations using RNN...           │       │  [A]dd [F]ind│
│  3 │▶ Hochreiter & Schmidhuber, 1997 — Long │ DOI   │ ✗ Not found  │
│    │ Short-Term Memory                       │       │  [A]dd [F]ind│
│  4 │ Sutskever et al., 2014 — Sequence to   │ arXiv │ ✓ In library │
│    │ Sequence Learning...                    │       │              │
│  5 │ Vinyals et al., 2015 — Grammar as a    │ arXiv │ ✗ Not found  │
│    │ Foreign Language                        │       │  [A]dd [F]ind│
├────┴─────────────────────────────────────────┴───────┴──────────────┤
│ [Enter] open/preview  [A] add to library  [F] find online           │
│ [AI] resolve all unresolved  [AA] add all to library  [E] export    │
└──────────────────────────────────────────────────────────────────────┘
```

### 2.5 Граф цитирований

```rust
pub struct CitationGraph {
    /// Входящие цитирования (кто цитирует эту работу)
    pub cited_by: Vec<CitationEdge>,
    /// Исходящие ссылки (что цитирует эта работа)
    pub references: Vec<CitationEdge>,
    /// Совместные цитирования (co-cited)
    pub co_citations: Vec<(BookId, u32)>,
    /// Bibliographic coupling (работы с похожим списком ссылок)
    pub bibliographic_coupling: Vec<(BookId, u32)>,
}

pub struct CitationEdge {
    pub source_id: Option<BookId>,   // Если есть в нашей библиотеке
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub citation_context: Option<String>,  // Фрагмент текста вокруг цитирования
    pub is_influential: bool,              // Semantic Scholar influential citation
}
```

**Визуализация графа в TUI:**

```
┌─────────────────────────────────────────────────────────────────────┐
│  CITATION GRAPH — "Attention Is All You Need"                       │
├─────────────────────────────────────────────────────────────────────┤
│  Mode: [References] [Cited By] [Related]                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ◉ Attention Is All You Need (2017)                                 │
│  ├── cites ──────────────────────────────────────────────────────── │
│  │   ├── [✓] Bahdanau 2014 — Attention mechanism         [arXiv]   │
│  │   ├── [✗] Hochreiter 1997 — LSTM                      [DOI]     │
│  │   └── [✓] Sutskever 2014 — Seq2Seq                    [arXiv]   │
│  │                                                                   │
│  └── cited by (87,654 papers) ─────────────────────────────────── │
│      ├── [✓] BERT (Devlin 2018)                           [arXiv]  │
│      ├── [✗] GPT (Radford 2018)                           [OpenAI] │
│      └── [✗] T5 (Raffel 2019)                             [arXiv]  │
│                                                                     │
│  Related (bibliographic coupling):                                  │
│      ├── [✗] Transformer-XL (Dai 2019)                   [arXiv]  │
│      └── [✓] Universal Transformers (Dehghani 2018)      [arXiv]  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Внешние источники — детальная реализация

### 3.1 Архитектура источников

```rust
/// Унифицированный интерфейс для всех внешних источников
#[async_trait]
pub trait ExternalSource: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> SourceType;        // Search | Metadata | Download
    fn requires_auth(&self) -> bool;
    fn rate_limit(&self) -> RateLimit;

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    async fn fetch_metadata(&self, id: &Identifier) -> Result<Option<Metadata>>;
    async fn find_download_url(&self, id: &Identifier) -> Result<Option<DownloadUrl>>;
    async fn health_check(&self) -> SourceStatus;
}

pub enum SourceType {
    AcademicMetadata,   // CrossRef, Semantic Scholar, OpenAlex
    BookMetadata,       // Open Library, Google Books
    Search,             // Anna's Archive, Sci-Hub lookup
    Download,           // Direct download sources
    OpenAccess,         // Unpaywall, CORE
}

pub struct SourceStatus {
    pub available: bool,
    pub latency_ms: u64,
    pub last_checked: DateTime<Utc>,
    pub mirror: Option<String>,     // Для зеркал Sci-Hub/Anna's
}
```

### 3.2 Anna's Archive — реализация

```rust
/// Поиск и получение ссылок с Anna's Archive без официального API
pub mod annas_archive {

    // Публичные инстанции (актуальные зеркала)
    const MIRRORS: &[&str] = &[
        "https://annas-archive.org",
        "https://annas-archive.se",
        "https://annas-archive.li",
        "https://annas-archive.gs",
    ];

    pub struct AnnasArchiveSource {
        client: RateLimitedClient,
        active_mirror: RwLock<&'static str>,
        cache: DiskCache<String, Vec<SearchResult>>,
    }

    impl AnnasArchiveSource {
        pub fn new() -> Self {
            Self {
                client: RateLimitedClient::new(
                    Duration::from_secs(2),   // 1 запрос в 2 секунды
                    3,                        // 3 попытки при ошибке
                ),
                active_mirror: RwLock::new(MIRRORS[0]),
                cache: DiskCache::new("annas_archive", Duration::from_hours(24)),
            }
        }

        /// Поиск через HTML-скрейпинг
        pub async fn search(&self, query: &AnnasQuery) -> Result<Vec<AnnasResult>> {
            // Построение URL:
            // https://annas-archive.org/search?q={query}&ext=pdf,epub,djvu&lang=en
            let url = self.build_search_url(query);

            // Проверить кэш
            if let Some(cached) = self.cache.get(&url).await {
                return Ok(cached);
            }

            // HTTP запрос с ротацией зеркал при ошибке
            let html = self.fetch_with_mirror_rotation(&url).await?;

            // Парсинг HTML результатов
            let results = self.parse_search_html(&html)?;

            // Кэшировать
            self.cache.set(&url, &results).await;

            Ok(results)
        }

        /// Парсинг страницы поиска Anna's Archive
        fn parse_search_html(&self, html: &str) -> Result<Vec<AnnasResult>> {
            let document = Html::parse_document(html);

            // Селекторы для карточек результатов
            let card_selector = Selector::parse("div.h-[125px]").unwrap();
            let title_selector = Selector::parse("h3").unwrap();
            let meta_selector = Selector::parse("div.text-sm").unwrap();

            document.select(&card_selector).map(|card| {
                let title = card.select(&title_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();

                // Парсим мета-информацию (формат, размер, язык, год)
                let meta_text = card.select(&meta_selector)
                    .map(|e| e.text().collect::<String>())
                    .collect::<Vec<_>>();

                // Извлечь MD5 из URL (ключ к файлу на Anna's Archive)
                let md5 = card.select(&Selector::parse("a").unwrap())
                    .next()
                    .and_then(|a| a.value().attr("href"))
                    .and_then(|href| extract_md5_from_url(href));

                Ok(AnnasResult {
                    title,
                    authors: parse_authors_from_meta(&meta_text),
                    year: parse_year_from_meta(&meta_text),
                    format: parse_format_from_meta(&meta_text),
                    size_mb: parse_size_from_meta(&meta_text),
                    language: parse_lang_from_meta(&meta_text),
                    md5,
                    detail_url: build_detail_url(md5.as_deref()),
                    isbn: parse_isbn_from_meta(&meta_text),
                })
            }).collect()
        }

        /// Получить прямые ссылки на скачивание через страницу детали
        pub async fn get_download_links(&self, md5: &str) -> Result<Vec<DownloadLink>> {
            let url = format!("{}/md5/{}", self.active_mirror(), md5);
            let html = self.fetch_with_mirror_rotation(&url).await?;

            // На странице MD5 есть несколько источников:
            // - Libgen.li
            // - Libgen.rs  
            // - Z-Library mirrors
            // - IPFS
            // - Напрямую с Anna's Archive (медленно)
            self.parse_download_links(&html)
        }
    }

    pub struct AnnasQuery {
        pub q: String,
        pub ext: Vec<String>,           // ["pdf", "epub", "djvu"]
        pub lang: Option<String>,       // "en", "ru", etc.
        pub content: Option<String>,    // "book_any", "book_nonfiction", "journal_article"
        pub isbn: Option<String>,
        pub doi: Option<String>,
        pub sort: AnnaSort,             // relevance | year | size
    }

    pub struct AnnasResult {
        pub title: String,
        pub authors: Vec<String>,
        pub year: Option<i32>,
        pub format: String,
        pub size_mb: f64,
        pub language: String,
        pub md5: Option<String>,        // Ключ на Anna's Archive
        pub detail_url: String,
        pub isbn: Option<String>,
        pub publisher: Option<String>,
        pub cover_url: Option<String>,
    }

    pub struct DownloadLink {
        pub source: String,             // "libgen.li", "z-lib", "ipfs"
        pub url: String,
        pub priority: u8,               // Скорость/надёжность
    }
}
```

### 3.3 Sci-Hub — реализация

```rust
/// Sci-Hub: поиск по DOI, PMID, URL статьи
pub mod scihub {

    // Известные рабочие зеркала (могут меняться)
    // Программа проверяет доступность при старте и кэширует рабочий
    const KNOWN_MIRRORS: &[&str] = &[
        "https://sci-hub.se",
        "https://sci-hub.st",
        "https://sci-hub.ru",
        "https://sci-hub.ren",
        "https://sci-hub.mksa.top",
    ];

    pub struct SciHubSource {
        client: RateLimitedClient,
        working_mirror: RwLock<Option<String>>,
        mirror_cache: DiskCache<(), Vec<String>>,
    }

    impl SciHubSource {
        /// Найти рабочее зеркало
        pub async fn find_working_mirror(&self) -> Result<String> {
            // Попробовать известные зеркала
            for mirror in KNOWN_MIRRORS {
                if self.check_mirror(mirror).await.is_ok() {
                    return Ok(mirror.to_string());
                }
            }

            // Fallback: получить актуальные зеркала с Wikipedia или известных источников
            self.fetch_mirrors_from_community().await
        }

        /// Получить PDF по DOI
        pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<PaperInfo> {
            let mirror = self.get_or_find_mirror().await?;
            let url = format!("{}/{}", mirror, doi.normalized);

            let html = self.client.get(&url).await?;
            self.parse_scihub_page(&html, doi)
        }

        /// Парсинг страницы Sci-Hub
        fn parse_scihub_page(&self, html: &str, doi: &Doi) -> Result<PaperInfo> {
            let document = Html::parse_document(html);

            // Найти iframe или embed с PDF
            let pdf_url = document
                .select(&Selector::parse("#pdf").unwrap())
                .next()
                .and_then(|el| el.value().attr("src"))
                .map(|src| normalize_sci_hub_url(src));

            // Заголовок статьи
            let title = document
                .select(&Selector::parse("#citation").unwrap())
                .next()
                .map(|el| el.text().collect::<String>());

            Ok(PaperInfo {
                doi: doi.clone(),
                pdf_url,
                title,
                direct_download: pdf_url.map(|u| u.replace("//", "https://")),
            })
        }

        /// Поддерживаемые форматы ввода
        pub async fn fetch(&self, identifier: &SciHubIdentifier) -> Result<PaperInfo> {
            match identifier {
                SciHubIdentifier::Doi(doi) => self.fetch_by_doi(doi).await,
                SciHubIdentifier::Pmid(pmid) => self.fetch_by_pmid(*pmid).await,
                SciHubIdentifier::Url(url) => self.fetch_by_url(url).await,
                SciHubIdentifier::ArxivId(id) => {
                    // arXiv статьи обычно открытые — сначала попробовать напрямую
                    if let Ok(pdf) = arxiv_direct_pdf(id).await {
                        return Ok(pdf);
                    }
                    // Fallback на Sci-Hub если arxiv недоступен
                    self.fetch_by_arxiv_doi(id).await
                }
            }
        }
    }

    pub enum SciHubIdentifier {
        Doi(Doi),
        Pmid(u32),
        Url(String),            // Прямой URL страницы статьи
        ArxivId(ArxivId),
    }
}
```

### 3.4 TUI для поиска и загрузки

```
┌─────────────────────────────────────────────────────────────────────┐
│  🌐 FIND & DOWNLOAD                    Sources: [A✓] [S✓] [O✓] [G✓]│
├─────────────────────────────────────────────────────────────────────┤
│  > attention mechanism transformer_                                  │
│  ── or search by: [DOI] [arXiv] [ISBN] [PMID] ──────────────────── │
├──────────────────────────────────────────────────────────────────────┤
│  Anna's Archive (5)            │  Semantic Scholar (10)             │
│                                │                                    │
│  ▶ Attention Is All You Need   │  ▶ Attention Is All You Need       │
│    Vaswani et al. 2017         │    arXiv:1706.03762                │
│    PDF 1.2MB  EN  ★★★★         │    87,654 citations                │
│    [D]ownload [M]eta [↗]open  │    Open Access ✓                   │
│                                │    [A]dd [D]ownload [↗]           │
│    An Image is Worth 16x16..   │                                    │
│    Dosovitskiy 2020            │  ▶ Neural Machine Translation by   │
│    PDF 2.1MB  EN               │    Jointly Learning to Align...    │
│                                │    arXiv:1409.0473  ✓ In library  │
│  Sci-Hub                       │                                    │
│  [Enter DOI/PMID/URL to search]│  OpenAlex (3 open access)         │
│                                │  ▶ Self-Attention with Relative... │
└────────────────────────────────┴────────────────────────────────────┘
│ Sources: [A]nna's Archive  [S]ci-Hub  [O]pen Library  [G]Books     │
│ [Tab] switch source  [D] download  [M] import metadata  [Esc] close│
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Академические базы данных — источники метаданных

### 4.1 CrossRef — основной источник DOI-метаданных

```rust
pub mod crossref {
    // Бесплатный API, рекомендуется указать email в User-Agent
    // для "polite pool" с более высокими лимитами
    const BASE_URL: &str = "https://api.crossref.org";
    const POLITE_POOL_EMAIL: &str = "omniscope-user@example.com"; // из конфига

    /// Получить метаданные по DOI
    pub async fn fetch_by_doi(doi: &Doi) -> Result<CrossRefWork> {
        let url = format!("{BASE_URL}/works/{}", doi.normalized);
        http_get_json(&url).await
    }

    /// Text Query API — найти работу по тексту ссылки
    pub async fn query_by_text(reference_string: &str) -> Result<Vec<CrossRefWork>> {
        let url = format!("{BASE_URL}/works?query.bibliographic={}&rows=5",
                          urlenccode(reference_string));
        http_get_json(&url).await
    }

    /// Batch resolution — до 100 DOI за раз
    pub async fn fetch_batch(dois: &[Doi]) -> Result<Vec<CrossRefWork>> {
        // Используем фильтры и select для экономии трафика
        let doi_filter = dois.iter().map(|d| &d.normalized).join(",");
        let select = "DOI,title,author,published,type,container-title,ISSN,ISBN,publisher";
        let url = format!("{BASE_URL}/works?filter=doi:{doi_filter}&select={select}");
        http_get_json(&url).await
    }

    pub struct CrossRefWork {
        pub doi: String,
        pub title: Vec<String>,
        pub author: Vec<CrossRefAuthor>,
        pub published: Option<CrossRefDate>,
        pub work_type: String,          // "journal-article" | "book" | "proceedings-article"
        pub container_title: Vec<String>, // Журнал/конференция
        pub publisher: Option<String>,
        pub issn: Vec<String>,
        pub isbn: Vec<String>,
        pub url: Option<String>,
        pub abstract_text: Option<String>,
        pub references_count: Option<u32>,
        pub is_referenced_by_count: Option<u32>,
        pub license: Vec<CrossRefLicense>,
        pub link: Vec<CrossRefLink>,    // Ссылки на PDF/HTML
    }
}
```

### 4.2 Semantic Scholar — граф цитирований

```rust
pub mod semantic_scholar {
    // Бесплатный API, rate limit: 100 req/5min без ключа, 1 req/sec с ключом
    const BASE_URL: &str = "https://api.semanticscholar.org/graph/v1";

    /// Получить данные по Paper ID (S2 ID, DOI, arXiv ID)
    pub async fn fetch_paper(id: &PaperId, fields: &[S2Field]) -> Result<S2Paper> {
        // fields: title,authors,year,abstract,externalIds,
        //         citationCount,referenceCount,influentialCitationCount,
        //         fieldsOfStudy,s2FieldsOfStudy,isOpenAccess,openAccessPdf,
        //         citations,references,embedding,tldr
        let fields_str = fields.iter().map(|f| f.as_str()).join(",");
        let url = format!("{BASE_URL}/paper/{id}?fields={fields_str}");
        http_get_json(&url).await
    }

    /// Пакетный запрос (до 500 ID за раз)
    pub async fn fetch_batch(ids: &[PaperId], fields: &[S2Field]) -> Result<Vec<S2Paper>> {
        let body = json!({ "ids": ids.iter().map(|i| i.to_string()).collect::<Vec<_>>() });
        let fields_str = fields.iter().map(|f| f.as_str()).join(",");
        http_post_json(&format!("{BASE_URL}/paper/batch?fields={fields_str}"), &body).await
    }

    /// Рекомендации похожих статей
    pub async fn get_recommendations(paper_id: &str) -> Result<Vec<S2Paper>> {
        let url = format!(
            "https://api.semanticscholar.org/recommendations/v1/papers/forpaper/{}?limit=20",
            paper_id
        );
        http_get_json(&url).await
    }

    /// Поиск авторов
    pub async fn search_author(name: &str) -> Result<Vec<S2Author>> {
        let url = format!("{BASE_URL}/author/search?query={}&fields=name,paperCount,citationCount,hIndex",
                          urlenccode(name));
        http_get_json(&url).await
    }

    pub struct S2Paper {
        pub paper_id: String,
        pub external_ids: HashMap<String, String>,  // DOI, ArXiv, PubMed, MAG...
        pub title: String,
        pub abstract_text: Option<String>,
        pub year: Option<i32>,
        pub authors: Vec<S2Author>,
        pub citation_count: u32,
        pub reference_count: u32,
        pub influential_citation_count: u32,
        pub fields_of_study: Vec<String>,
        pub is_open_access: bool,
        pub open_access_pdf: Option<S2OpenAccessPdf>,
        pub tldr: Option<S2Tldr>,          // AI-generated TL;DR
        pub embedding: Option<Vec<f32>>,    // Semantic embedding
        pub citations: Vec<S2PaperRef>,
        pub references: Vec<S2PaperRef>,
    }

    pub struct S2Tldr {
        pub model: String,
        pub text: String,   // Однострочное AI-резюме
    }
}
```

### 4.3 OpenAlex — открытая альтернатива

```rust
pub mod openalex {
    // Полностью открытый API, без ключа, rate limit: 10 req/sec
    // Snapshot: 250M+ работ, 100% открытые данные
    const BASE_URL: &str = "https://api.openalex.org";

    pub async fn fetch_work(id: &OpenAlexId) -> Result<OpenAlexWork> {
        // Поддерживает форматы:
        // W2741809807 (OpenAlex ID)
        // doi:10.1000/xyz (DOI)
        // pmid:12345678 (PubMed)
        // mag:12345678 (MAG)
        let url = format!("{BASE_URL}/works/{id}");
        http_get_json(&url).await
    }

    /// Поиск с полным фильтром OpenAlex
    pub async fn search(filter: &OpenAlexFilter) -> Result<OpenAlexPage<OpenAlexWork>> {
        // Поддерживаемые фильтры:
        // title.search, abstract.search, fulltext.search
        // author.id, author.orcid, author.display_name
        // publication_year, cited_by_count
        // primary_location.source.id (журнал)
        // open_access.is_oa, open_access.oa_status
        // concepts.id, keywords.keyword
        // type: article | book | dataset | dissertation
        let url = format!("{BASE_URL}/works?{}", filter.to_query_string());
        http_get_json(&url).await
    }

    pub struct OpenAlexWork {
        pub id: String,                     // W2741809807
        pub doi: Option<String>,
        pub title: String,
        pub display_name: String,
        pub publication_year: Option<i32>,
        pub publication_date: Option<String>,
        pub ids: OpenAlexIds,               // Все доступные ID
        pub primary_location: Option<Location>,
        pub open_access: OpenAccessInfo,
        pub authorships: Vec<Authorship>,
        pub cited_by_count: u32,
        pub referenced_works: Vec<String>,  // OpenAlex IDs
        pub related_works: Vec<String>,
        pub concepts: Vec<Concept>,         // Тематические концепции
        pub keywords: Vec<Keyword>,
        pub abstract_inverted_index: Option<HashMap<String, Vec<u32>>>, // Инвертированный индекс
    }
}
```

### 4.4 Unpaywall — Open Access поиск

```rust
pub mod unpaywall {
    // Бесплатный API, требует только email
    const BASE_URL: &str = "https://api.unpaywall.org/v2";

    /// Проверить открытый доступ для DOI
    pub async fn check_oa(doi: &Doi, email: &str) -> Result<UnpaywallResult> {
        let url = format!("{BASE_URL}/{}?email={}", doi.normalized, email);
        http_get_json(&url).await
    }

    pub struct UnpaywallResult {
        pub doi: String,
        pub is_oa: bool,
        pub oa_status: OaStatus,   // gold | hybrid | bronze | green | closed
        pub best_oa_location: Option<OaLocation>,
        pub oa_locations: Vec<OaLocation>,
        pub updated: String,
        pub journal_is_oa: bool,
        pub journal_is_in_doaj: bool,
    }

    pub struct OaLocation {
        pub url: String,            // Ссылка на PDF
        pub url_for_pdf: Option<String>,
        pub url_for_landing_page: Option<String>,
        pub host_type: String,      // "publisher" | "repository"
        pub license: Option<String>, // "cc-by", "cc-by-nc", etc.
        pub version: String,        // "publishedVersion" | "acceptedVersion" | "submittedVersion"
        pub repository_institution: Option<String>,
    }
}
```

### 4.5 CORE — открытый репозиторий

```rust
pub mod core_ac {
    // https://core.ac.uk/api/v3/ — 250M+ открытых статей
    // Требует API ключ (бесплатный)

    pub async fn search(query: &str) -> Result<Vec<CoreWork>> {
        let url = format!(
            "https://api.core.ac.uk/v3/search/works?q={}&limit=10",
            urlenccode(query)
        );
        http_get_json_with_key(&url, &config().core_api_key).await
    }

    pub async fn fetch_by_doi(doi: &Doi) -> Result<Option<CoreWork>> {
        let url = format!("https://api.core.ac.uk/v3/works/doi:{}", doi.normalized);
        http_get_json_with_key(&url, &config().core_api_key).await
    }
}
```

---

## 5. Форматы библиографических данных

### 5.1 BibTeX — полная поддержка

```rust
pub mod bibtex {

    /// Парсинг BibTeX файла
    pub fn parse(content: &str) -> Result<Vec<BibEntry>> { /* ... */ }

    /// Генерация BibTeX из BookCard
    pub fn from_book_card(card: &BookCard) -> BibEntry {
        BibEntry {
            entry_type: card_type_to_bibtex(&card.publication.pub_type),
            cite_key: generate_cite_key(card),
            fields: build_bibtex_fields(card),
        }
    }

    /// Генерация cite key по стандартным схемам
    fn generate_cite_key(card: &BookCard) -> String {
        // Схемы (настраиваемые):
        // author_year: "Vaswani2017"
        // author_year_title: "Vaswani2017Attention"
        // doi_based: "10.48550/arXiv.1706.03762" → "arXiv1706.03762"
        // custom template: "{first_author}{year}{title_word}"
        match config().cite_key_scheme {
            CiteKeyScheme::AuthorYear => format!(
                "{}{}",
                first_author_last_name(card),
                card.metadata.year.unwrap_or(0)
            ),
            // ...
        }
    }

    /// Поддерживаемые типы записей BibTeX
    pub enum BibType {
        Article,        // Журнальная статья
        Book,           // Книга
        InProceedings,  // Статья в материалах конференции
        Proceedings,    // Материалы конференции
        InBook,         // Глава в книге
        InCollection,   // Часть коллекции
        PhDThesis,      // Докторская диссертация
        MasterThesis,   // Магистерская
        TechReport,     // Технический отчёт
        Unpublished,    // Неопубликованная работа (препринт)
        Misc,           // Прочее (сайты, программы)
        Manual,         // Техническая документация
        Booklet,        // Брошюра
        Conference,     // Псевдоним InProceedings
        Online,         // Онлайн-ресурс (biblatex)
        Preprint,       // Препринт (biblatex)
        Software,       // Программное обеспечение (biblatex)
        Dataset,        // Датасет (biblatex)
    }

    /// Пример BibTeX вывода
    // @article{Vaswani2017Attention,
    //   title     = {Attention Is All You Need},
    //   author    = {Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and
    //                Uszkoreit, Jakob and Jones, Llion and Gomez, Aidan N and
    //                Kaiser, Lukasz and Polosukhin, Illia},
    //   journal   = {Advances in Neural Information Processing Systems},
    //   volume    = {30},
    //   year      = {2017},
    //   doi       = {10.48550/arXiv.1706.03762},
    //   arxivid   = {1706.03762},
    //   eprint    = {1706.03762},
    //   archiveprefix = {arXiv},
    //   primaryclass = {cs.LG}
    // }
}
```

### 5.2 CSL (Citation Style Language) — Citation Processor

```rust
pub mod csl {
    // Поддержка 10,000+ стилей цитирования через citation-js совместимый движок
    // Стили: APA, MLA, Chicago, IEEE, Vancouver, Harvard, Nature, Science...

    pub struct CslProcessor {
        styles: HashMap<String, CslStyle>,  // Кэш стилей
        locale: String,                     // "en-US", "ru-RU"
    }

    impl CslProcessor {
        /// Сформатировать ссылку в нужном стиле
        pub fn format_citation(&self, card: &BookCard, style: &str) -> Result<String> {
            // APA: Vaswani, A., Shazeer, N., et al. (2017). Attention is all you need.
            //      Advances in Neural Information Processing Systems, 30.
            //
            // IEEE: A. Vaswani et al., "Attention Is All You Need,"
            //       in Adv. Neural Inf. Process. Syst., 2017, vol. 30.
            //
            // Chicago: Vaswani, Ashish, et al. "Attention Is All You Need."
            //          Advances in Neural Information Processing Systems 30 (2017).
            let csl_data = card_to_csl_item(card);
            format_with_style(&csl_data, style, &self.locale)
        }

        /// Форматировать список литературы
        pub fn format_bibliography(
            &self,
            cards: &[&BookCard],
            style: &str
        ) -> Result<Vec<String>> { /* ... */ }
    }

    /// Поддерживаемые стили (встроенные)
    pub const BUNDLED_STYLES: &[&str] = &[
        "apa",          "apa-6th-edition",
        "ieee",
        "mla",          "mla-8th-edition",
        "chicago-author-date", "chicago-note-bibliography",
        "harvard-cite-them-right",
        "vancouver",    "vancouver-superscript",
        "nature",       "science",
        "cell",         "lancet",
        "gost-r-7-0-5-2008",  // Российский ГОСТ
        "russian-gost-r-7-0-5-2008",
    ];
    // Любые другие стили загружаются из ~/.local/share/omniscope/csl-styles/
}
```

### 5.3 RIS, EndNote, RDF — форматы импорта/экспорта

```rust
pub mod bibliography_formats {

    pub enum Format {
        BibTeX,
        BibLaTeX,       // Расширенный BibTeX
        RIS,            // Research Information Systems — EndNote, Mendeley
        EndNoteXml,     // XML формат EndNote
        ZoteroRdf,      // RDF формат Zotero
        ZoteroJson,     // JSON формат Zotero (CSL JSON)
        CslJson,        // Citation Style Language JSON
        ModsXml,        // MODS XML (Library of Congress)
        DublinCore,     // Dublin Core XML
        Marc21,         // MARC 21 (библиотечный стандарт)
        Csv,            // CSV с настраиваемым маппингом
        Nbib,           // PubMed NBIB формат
        Wos,            // Web of Science
        Scopus,         // Scopus CSV
    }

    /// RIS формат — используется в Mendeley, RefWorks
    // TY  - JOUR           <- тип: JOUR=журнал, BOOK=книга, CONF=конференция
    // TI  - Attention Is All You Need
    // AU  - Vaswani, Ashish
    // AU  - Shazeer, Noam
    // PY  - 2017
    // DO  - 10.48550/arXiv.1706.03762
    // UR  - https://arxiv.org/abs/1706.03762
    // ER  -               <- конец записи
}
```

---

## 6. Метаданные и обогащение

### 6.1 Пайплайн автоматического обогащения

```rust
/// Полный пайплайн обогащения метаданных
pub async fn enrich_metadata_pipeline(card: &mut BookCard) -> EnrichmentReport {
    let mut report = EnrichmentReport::new();

    // ── ЭТАП 1: Извлечение из файла ──────────────────────────────────

    if let Some(file) = &card.file {
        match file.format {
            FileFormat::Pdf => {
                // PDF метаданные (XMP, DocumentInfo)
                if let Ok(pdf_meta) = extract_pdf_metadata(&file.path) {
                    card.merge_metadata(pdf_meta, Source::PdfInternal);
                    report.add("PDF metadata extracted");
                }

                // XMP Dublin Core и прочие схемы
                if let Ok(xmp) = extract_xmp_metadata(&file.path) {
                    // XMP может содержать DOI, ISBN, авторов
                    if let Some(doi) = xmp.doi {
                        card.identifiers.doi = Some(doi);
                        report.add("DOI found in XMP");
                    }
                }

                // Попытка найти DOI в тексте первой страницы
                if let Ok(doi) = find_doi_in_first_page(&file.path) {
                    if card.identifiers.doi.is_none() {
                        card.identifiers.doi = Some(doi);
                        report.add("DOI found in first page text");
                    }
                }

                // Поиск arXiv ID в тексте
                if let Ok(arxiv_id) = find_arxiv_id_in_pdf(&file.path) {
                    card.identifiers.arxiv_id = Some(arxiv_id);
                    report.add("arXiv ID found in PDF text");
                }
            }
            FileFormat::Epub => {
                // EPUB OPF метаданные (Dublin Core)
                if let Ok(opf) = extract_epub_metadata(&file.path) {
                    card.merge_metadata(opf, Source::EpubOpf);
                }
            }
            FileFormat::Djvu => {
                // DjVu аннотации
                if let Ok(djvu_meta) = extract_djvu_metadata(&file.path) {
                    card.merge_metadata(djvu_meta, Source::DjvuAnnotation);
                }
            }
            _ => {}
        }
    }

    // ── ЭТАП 2: Обогащение через идентификаторы ───────────────────────

    // DOI → CrossRef (самый надёжный источник)
    if let Some(doi) = &card.identifiers.doi {
        if let Ok(crossref_data) = crossref::fetch_by_doi(doi).await {
            card.merge_metadata(crossref_data, Source::CrossRef);
            report.add("Enriched from CrossRef via DOI");
        }
    }

    // arXiv ID → arXiv API
    if let Some(arxiv_id) = &card.identifiers.arxiv_id {
        if let Ok(arxiv_data) = arxiv_api::fetch_metadata(arxiv_id).await {
            card.merge_metadata(arxiv_data, Source::ArxivApi);

            // Получить DOI из arXiv если не был
            if card.identifiers.doi.is_none() {
                card.identifiers.doi = arxiv_data.doi;
            }
        }
    }

    // ISBN → Open Library + Google Books
    if !card.identifiers.isbn.is_empty() {
        if let Ok(ol_data) = openlibrary::fetch_by_isbn(&card.identifiers.isbn[0]).await {
            card.merge_metadata(ol_data, Source::OpenLibrary);
            report.add("Enriched from Open Library via ISBN");
        }
    }

    // ── ЭТАП 3: Semantic Scholar (граф цитирований) ───────────────────

    let s2_id = card.identifiers.s2_paper_id.clone()
        .or_else(|| card.identifiers.doi.as_ref().map(|d| format!("DOI:{}", d.normalized)))
        .or_else(|| card.identifiers.arxiv_id.as_ref().map(|a| format!("ArXiv:{}", a.id)));

    if let Some(id) = s2_id {
        let fields = vec![
            S2Field::CitationCount, S2Field::ReferenceCount,
            S2Field::InfluentialCitationCount, S2Field::FieldsOfStudy,
            S2Field::IsOpenAccess, S2Field::OpenAccessPdf,
            S2Field::Tldr, S2Field::ExternalIds,
        ];
        if let Ok(s2_data) = semantic_scholar::fetch_paper(&id, &fields).await {
            card.citation_graph.citation_count = s2_data.citation_count;
            card.citation_graph.influential_citation_count = s2_data.influential_citation_count;
            card.ai.tldr = s2_data.tldr.map(|t| t.text);

            // Обновить все найденные ID
            for (key, val) in &s2_data.external_ids {
                match key.as_str() {
                    "MAG" => card.identifiers.mag_id = val.parse().ok(),
                    "PubMed" => card.identifiers.pmid = val.parse().ok(),
                    "DBLP" => card.identifiers.dblp_key = Some(val.clone()),
                    _ => {}
                }
            }
        }
    }

    // ── ЭТАП 4: Open Access поиск ─────────────────────────────────────

    if let Some(doi) = &card.identifiers.doi {
        if let Ok(oa_data) = unpaywall::check_oa(doi, &config().email).await {
            card.open_access = Some(OpenAccessInfo::from_unpaywall(oa_data));
            report.add("Open Access status checked");
        }
    }

    report
}
```

### 6.2 Разрешение конфликтов при слиянии данных

```rust
/// Приоритеты источников (выше = надёжнее)
pub fn source_priority(source: Source) -> u8 {
    match source {
        Source::UserManual => 100,      // Пользователь всегда прав
        Source::CrossRef => 90,         // Официальные DOI-метаданные
        Source::ArxivApi => 85,         // Официальный arXiv
        Source::PdfInternal => 80,      // Встроенные в PDF (если заполнены)
        Source::EpubOpf => 75,          // EPUB Dublin Core
        Source::OpenLibrary => 70,
        Source::SemanticScholar => 65,
        Source::OpenAlex => 60,
        Source::GoogleBooks => 55,
        Source::AiInferred => 40,       // AI-предположение
        Source::AnnasArchive => 30,     // Парсинг сторонних сайтов
        Source::Unknown => 10,
    }
}

/// Стратегия слияния
pub enum MergeStrategy {
    HighestPriority,        // Взять из самого приоритетного источника
    Concat,                 // Объединить списки (авторы, теги)
    Longest,                // Взять самый длинный (abstract)
    UserOverride,           // Никогда не перезаписывать пользовательские данные
}
```

---

## 7. Типы документов

### 7.1 Полная таксономия

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DocumentType {
    // ── КНИГИ ──────────────────────────────────────────────────────
    Book,               // Монография
    BookChapter,        // Глава в книге
    Textbook,           // Учебник
    ReferenceBook,      // Справочник/словарь
    EditedVolume,       // Коллективная монография (под редакцией)

    // ── ПЕРИОДИКА ──────────────────────────────────────────────────
    JournalArticle,     // Статья в журнале
    ReviewArticle,      // Обзорная статья
    ShortCommunication, // Краткое сообщение / letter
    Editorial,          // Редакционная статья

    // ── КОНФЕРЕНЦИИ ────────────────────────────────────────────────
    ConferencePaper,    // Статья в материалах конференции
    ConferencePoster,   // Постер конференции
    ConferenceTalk,     // Доклад (без полной статьи)

    // ── ПРЕПРИНТЫ И РАБОЧИЕ БУМАГИ ───────────────────────────────
    Preprint,           // Препринт (arXiv, bioRxiv, SSRN)
    WorkingPaper,       // Рабочий документ
    TechnicalReport,    // Технический отчёт

    // ── ДИССЕРТАЦИИ ────────────────────────────────────────────────
    PhdThesis,          // Докторская диссертация
    MasterThesis,       // Магистерская диссертация
    BachelorThesis,     // Бакалаврская работа

    // ── СТАНДАРТЫ И НОРМАТИВНЫЕ ДОКУМЕНТЫ ────────────────────────
    Standard,           // ISO, IEEE, ANSI, ГОСТ
    Specification,      // Техническая спецификация (RFC, W3C)
    Patent,             // Патент

    // ── ДРУГОЕ ────────────────────────────────────────────────────
    Dataset,            // Набор данных
    Software,           // Программное обеспечение (с DOI)
    Presentation,       // Презентация
    Lecture,            // Конспект лекции
    Website,            // Веб-сайт/онлайн-ресурс
    BlogPost,           // Блог-пост
    Video,              // Учебное видео
    Podcast,            // Подкаст
    Manual,             // Руководство пользователя
    Documentation,      // Техническая документация
    Other(String),      // Пользовательский тип
}
```

---

## 8. Научный TUI — специализированные виды

### 8.1 Вид карточки статьи

```
┌──────────────────────────────────────────────────────────────────────┐
│  📄 Attention Is All You Need                    [preprint] [2017]  │
├──────────────────────────────────────────────────────────────────────┤
│  Authors:  Vaswani, A. · Shazeer, N. · Parmar, N. · (+5)           │
│  Venue:    NeurIPS 2017                                             │
│  Journal:  Advances in Neural Information Processing Systems, 30    │
├──────────────────────────────────────────────────────────────────────┤
│  IDENTIFIERS                                                         │
│  DOI:      10.48550/arXiv.1706.03762         [↗ open]               │
│  arXiv:    1706.03762v5  [cs.CL]             [↗ abs] [↗ pdf]        │
│  S2:       204e3073870fae3d05bcbc2f6a8e263d9b72e776                 │
│  OpenAlex: W2963403868                                              │
│  MAG:      2963403868                                               │
├──────────────────────────────────────────────────────────────────────┤
│  METRICS                                                             │
│  Citations: 87,654  (📈 +2,341 last month)                          │
│  Influential: 12,450  │  References: 41                             │
│  h-index impact: very high · Fields: cs.CL, cs.LG, cs.AI           │
├──────────────────────────────────────────────────────────────────────┤
│  OPEN ACCESS:  ✓ Green OA                                           │
│  PDF:    https://arxiv.org/pdf/1706.03762  [★ Best]                 │
│          https://proceedings.neurips.cc/...                         │
├──────────────────────────────────────────────────────────────────────┤
│  TL;DR (Semantic Scholar AI):                                        │
│  Introduces Transformer, a model based entirely on attention        │
│  mechanisms without recurrence or convolutions, showing superior    │
│  performance on translation tasks.                                  │
├──────────────────────────────────────────────────────────────────────┤
│  [o]pen  [r]eferences  [c]itations  [e]xport BibTeX  [ai]  [f]ind  │
└──────────────────────────────────────────────────────────────────────┘
```

### 8.2 Вид статистики библиотеки

```
┌──────────────────────────────────────────────────────────────────────┐
│  📊 LIBRARY STATISTICS                          [programming: 47]   │
├──────────────────────────────────────────────────────────────────────┤
│  BY TYPE                    BY FORMAT           BY YEAR              │
│  ■ Articles:    23 (49%)   ■ PDF:  38 (81%)   2024: ▓▓▓▓▓ 12       │
│  ■ Books:       15 (32%)   ■ EPUB:  7 (15%)   2023: ▓▓▓▓ 9         │
│  ■ Preprints:    6 (13%)   ■ DJVU:  2 (4%)    2022: ▓▓▓ 8          │
│  ■ Reports:      3 (6%)                        2021: ▓▓▓ 7          │
│                                                2020: ▓▓ 6           │
├──────────────────────────────────────────────────────────────────────┤
│  CITATION METRICS                                                    │
│  Most cited in library:                                             │
│  1. Attention Is All You Need    87,654 cit.                        │
│  2. BERT (Devlin 2018)           68,432 cit.                        │
│  3. ResNet (He 2016)             54,211 cit.                        │
│                                                                     │
│  Total citations across library: 1,247,893                         │
│  Average citations per paper: 26,551                               │
├──────────────────────────────────────────────────────────────────────┤
│  IDENTIFIERS COVERAGE                                               │
│  DOI:     43/47 (91%) ██████████████████░░                          │
│  arXiv:   28/47 (60%) ████████████░░░░░░░░                          │
│  ISBN:    15/47 (32%) ██████░░░░░░░░░░░░░░                          │
│  S2 ID:   35/47 (74%) ██████████████░░░░░░                          │
│  Open Access: 31/47 (66%) █████████████░░░░░░                       │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 9. CLI для научного модуля

```bash
# ── ARXIV ─────────────────────────────────────────────────────────────

# Добавить по arXiv ID
bshelf arxiv add 1706.03762
bshelf arxiv add 1706.03762v5 --download-pdf
bshelf arxiv add "https://arxiv.org/abs/1706.03762" --json

# Поиск в arXiv
bshelf arxiv search "attention transformer"
bshelf arxiv search --author "Vaswani" --category "cs.CL" --year 2017
bshelf arxiv search --title "attention" --max 20 --json

# Обновить версию препринта
bshelf arxiv update <id>          # Проверить новую версию
bshelf arxiv update --all         # Обновить все arXiv записи

# ── DOI ───────────────────────────────────────────────────────────────

# Добавить по DOI
bshelf doi add 10.1000/xyz123
bshelf doi add "https://doi.org/10.48550/arXiv.1706.03762"

# Разрешить DOI (получить метаданные)
bshelf doi resolve 10.48550/arXiv.1706.03762 --json

# ── ССЫЛКИ ────────────────────────────────────────────────────────────

# Извлечь ссылки из книги
bshelf refs extract <book-id>
bshelf refs extract <book-id> --json

# Показать ссылки
bshelf refs list <book-id>
bshelf refs list <book-id> --filter "not-in-library"

# Добавить все ненайденные ссылки
bshelf refs add-all <book-id>
bshelf refs add-all <book-id> --download-pdfs

# Граф цитирований
bshelf refs graph <book-id>           # ASCII граф
bshelf refs cited-by <book-id>        # Кто цитирует
bshelf refs related <book-id>         # Похожие статьи (S2)

# ── ЭКСПОРТ БИБЛИОГРАФИИ ──────────────────────────────────────────────

# Форматированная цитата
bshelf cite <book-id> --style apa
bshelf cite <book-id> --style ieee
bshelf cite <book-id> --style gost-r-7-0-5-2008

# Экспорт в BibTeX
bshelf export bibtex /path/to/refs.bib
bshelf export bibtex - --library ml-papers  # stdout
bshelf export bibtex - --tag transformer --json

# Экспорт в RIS
bshelf export ris /path/to/refs.ris

# Копировать цитату в буфер обмена
bshelf cite <book-id> --style apa --clipboard

# ── ПОИСК ОНЛАЙН ──────────────────────────────────────────────────────

# Поиск через Sci-Hub по DOI/PMID
bshelf fetch doi:10.1000/xyz123
bshelf fetch pmid:12345678
bshelf fetch "https://journal.com/article/..." # по URL страницы

# Проверить Open Access
bshelf oa check <book-id>
bshelf oa check doi:10.1000/xyz123 --json

# Найти PDF через Unpaywall
bshelf oa download <book-id>

# ── МЕТАДАННЫЕ ────────────────────────────────────────────────────────

# Обогатить метаданные
bshelf enrich <book-id>
bshelf enrich <book-id> --sources "crossref,s2,unpaywall"
bshelf enrich --all --json

# Проверить все идентификаторы
bshelf ids check <book-id> --json

# Добавить идентификатор вручную
bshelf ids set <book-id> --doi "10.1000/xyz"
bshelf ids set <book-id> --arxiv "1706.03762"
bshelf ids set <book-id> --isbn "9781718503106"
bshelf ids set <book-id> --pmid "12345678"

# Найти дубликаты по идентификаторам
bshelf dedup --by-doi
bshelf dedup --by-isbn
bshelf dedup --by-title-fuzzy
bshelf dedup --json

# ── СТАТИСТИКА ────────────────────────────────────────────────────────

bshelf stats science --json          # Полная научная статистика
bshelf stats citations --top 10      # Самые цитируемые
bshelf stats oa                      # Статистика открытого доступа
```

---

## 10. Конфигурация научного модуля

```toml
# ~/.config/omniscope/config.toml

[science]
# Email для "polite pool" CrossRef и Unpaywall (повышает лимиты)
# CrossRef: >50 req/sec в polite pool vs ~3 req/sec без
polite_pool_email = "your@email.com"

# API ключи (опционально, повышают лимиты)
semantic_scholar_api_key = ""    # https://www.semanticscholar.org/product/api
core_api_key = ""                # https://core.ac.uk/services/api
ncbi_api_key = ""                # https://www.ncbi.nlm.nih.gov/account/

# Автоматические действия
auto_extract_doi_from_pdf = true
auto_fetch_arxiv_metadata = true
auto_check_open_access = true
auto_extract_references = false    # Медленно, включать вручную

# Загрузка файлов
preferred_pdf_sources = [
    "unpaywall",           # Легальный открытый доступ — в первую очередь
    "arxiv_direct",        # Прямо с arXiv
    "core",                # CORE репозиторий
    "annas_archive",       # Если легальные источники не нашли
    "scihub",              # Последнее средство
]
download_directory = "~/Papers"
auto_rename_on_download = true    # Переименовать по схеме
rename_scheme = "{author}_{year}_{title_short}"

# Sci-Hub
[science.scihub]
enabled = true
mirror_check_on_startup = true
preferred_mirrors = []    # Пусто = автовыбор

# Anna's Archive
[science.annas_archive]
enabled = true
preferred_formats = ["pdf", "epub"]
preferred_languages = ["en", "ru"]

# Форматы экспорта
[science.export]
default_cite_style = "ieee"
cite_key_scheme = "author_year"    # author_year | doi_based | custom
bibtex_utf8 = true                 # UTF-8 в BibTeX (не escaped)

# Граф цитирований
[science.citation_graph]
fetch_on_add = false               # Получать граф при добавлении (медленно)
fetch_depth = 1                    # Глубина: 1 = только прямые ссылки
max_citations_to_store = 100       # Хранить N цитирований (сэмпл)
```

---

## 11. Vim Motions для научного модуля

Дополнение к основному vim-документу — специфичные для науки:

```
── НАУЧНЫЕ KEYBINDINGS (NORMAL mode) ──────────────────────────

gr          go references   → открыть панель ссылок статьи
gR          go cited-by     → кто цитирует эту статью
gs          go similar      → похожие статьи (S2 recommendations)
gd          go doi          → открыть DOI в браузере
ga          go arxiv        → открыть arXiv страницу
gA          go arXiv pdf    → открыть arXiv PDF напрямую
go          go open-access  → найти открытый PDF

yi          yank identifier → скопировать DOI/arXiv/ISBN
yD          yank doi        → скопировать только DOI
yA          yank arxiv      → скопировать arXiv ID
yB          yank bibtex     → скопировать BibTeX в буфер обмена
yC          yank citation   → скопировать форматированную цитату (default style)

cD          change doi      → установить/исправить DOI
cA          change arxiv    → установить arXiv ID

@e          AI: enrich metadata
@r          AI: extract and resolve references
@c          AI: generate citation recommendation

:cite       показать форматированную цитату
:cite ieee  в стиле IEEE
:bibtex     показать BibTeX
:refs       открыть список ссылок
:cited-by   открыть список цитирующих

── В ПАНЕЛИ ССЫЛОК ────────────────────────────────────────────

Enter       Перейти к книге (если в библиотеке) / показать детали
a           Add to library (добавить ссылку в библиотеку)
f           Find online (найти PDF)
A           Add all unresolved (добавить все ненайденные)
e           Export references (экспортировать список)
/           Поиск внутри списка ссылок
r           Resolve selected (попытаться разрешить ссылку)
```

---

*Научная составляющая omniscope превращает его из просто менеджера файлов в полноценную исследовательскую среду — аналог Zotero, но с философией vim и скоростью Rust.*
