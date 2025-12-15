# CLAUDE.md - Project Overview

## What is this project?

`bookexport` is a Rust CLI tool that extracts reading highlights from Apple Books and Kindle on macOS, merges/deduplicates books across sources, and outputs a unified JSON file.

## Project Structure

```
bookexport/
├── Cargo.toml              # Dependencies and project metadata
├── CLAUDE.md               # This file
└── src/
    ├── main.rs             # CLI entry point, argument parsing, orchestration
    ├── lib.rs              # Library re-exports
    ├── model.rs            # Data structures (Library, Book, Highlight, Source, Location)
    ├── error.rs            # Error types (AppleBooksError, KindleError, ConfigError)
    ├── apple_books.rs      # Apple Books SQLite extraction
    ├── kindle/
    │   ├── mod.rs          # Kindle module exports
    │   ├── clippings.rs    # My Clippings.txt parser
    │   └── scraper.rs      # Amazon Notebook web scraper
    ├── merge.rs            # Book/highlight deduplication logic
    └── config.rs           # TOML config file support
```

## Data Model

```rust
// The complete export
struct Library {
    exported_at: DateTime<Utc>,
    books: Vec<Book>,
}

// A book with metadata and highlights
struct Book {
    id: String,                    // SHA256(lowercase(title + author))[:16]
    title: String,
    author: Option<String>,
    sources: Vec<Source>,          // Which platforms this book was found on
    highlights: Vec<Highlight>,
    finished: Option<bool>,
    finished_at: Option<DateTime<Utc>>,
}

// A single highlight/annotation
struct Highlight {
    id: String,                    // From source DB or generated UUID
    text: String,
    note: Option<String>,
    location: Location,
    created_at: Option<DateTime<Utc>>,
    source: Source,
}

struct Location {
    chapter: Option<String>,
    position: Option<String>,      // Opaque string, format varies by source
}

enum Source {
    AppleBooks,
    Kindle,
}
```

## Data Sources

### Apple Books (macOS)

**Database Locations:**
- Library: `~/Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary/BKLibrary*.sqlite`
- Annotations: `~/Library/Containers/com.apple.iBooksX/Data/Documents/AEAnnotation/AEAnnotation*.sqlite`

**Key Tables:**
- `ZBKLIBRARYASSET` - Book metadata (title, author, finished status)
- `ZAEANNOTATION` - Highlights and notes

**Important Notes:**
- Timestamps are CoreData format (seconds since 2001-01-01). Convert with: `timestamp + 978307200` → Unix epoch
- Books and annotations are linked by `ZASSETID` ↔ `ZANNOTATIONASSETID`
- The code copies databases to temp location before reading to avoid SQLITE_BUSY errors

**SQL for Books:**
```sql
SELECT ZASSETID, ZTITLE, ZAUTHOR, ZISFINISHED, ZDATEFINISHED
FROM ZBKLIBRARYASSET
WHERE ZTITLE IS NOT NULL
```

**SQL for Annotations:**
```sql
SELECT ZANNOTATIONUUID, ZANNOTATIONASSETID, ZANNOTATIONSELECTEDTEXT,
       ZANNOTATIONNOTE, ZFUTUREPROOFING5, ZANNOTATIONLOCATION, ZANNOTATIONCREATIONDATE
FROM ZAEANNOTATION
WHERE ZANNOTATIONDELETED = 0
  AND ZANNOTATIONSELECTEDTEXT IS NOT NULL
  AND ZANNOTATIONSELECTEDTEXT != ''
ORDER BY ZANNOTATIONASSETID, ZPLLOCATIONRANGESTART
```

### Kindle - My Clippings.txt

**Location:** `/documents/My Clippings.txt` on Kindle device (via USB)

**Format:**
```
Book Title (Author Name)
- Your Highlight on Location 123-145 | Added on Monday, January 1, 2024

The actual highlighted text goes here...
==========
```

**Parsing:**
- Split by `==========` separator
- Extract title/author from first line via regex: `^(.+) \((.+)\)$`
- Parse location and date from second line
- Remaining lines are the highlight text

### Kindle - Amazon Notebook Scraping

**URL:** `https://read.amazon.com/notebook` (or regional variants)

**Authentication:** User exports browser cookies in Netscape format after logging into Amazon.

**CSS Selectors (from obsidian-kindle-plugin):**
- Book list: `.kp-notebook-library-each-book`
  - Title: `h2.kp-notebook-searchable`
  - Author: `p.kp-notebook-searchable`
  - ASIN: element `id` attribute
- Highlights: `.a-row.a-spacing-base`
  - Text: `#highlight`
  - Note: `#note`
  - Location: `#kp-annotation-location`
- Pagination: `.kp-notebook-annotations-next-page-start`, `.kp-notebook-content-limit-state`

**Regional Domains:**
- US: `amazon.com`
- UK: `amazon.co.uk`
- DE: `amazon.de`
- FR: `amazon.fr`
- JP: `amazon.co.jp`
- (and others)

**Note:** The macOS Kindle app (`com.amazon.Lassen`) stores only position markers in its local database, NOT the highlight text. This is why web scraping or My Clippings.txt is needed for Kindle highlights.

## CLI Usage

```
bookexport [OPTIONS]

Options:
    -o, --output <PATH>              Output path [default: ~/.local/share/bookexport/library.json]
    -c, --config <PATH>              Config file path [default: ~/.config/bookexport/config.toml]
    --apple-books-only               Only export from Apple Books
    --kindle-clippings <PATH>        Path to Kindle's My Clippings.txt file
    --kindle-cookies <PATH>          Path to exported Amazon cookies file (Netscape format)
    --kindle-region <REGION>         Amazon region: us, uk, de, fr, jp, etc. [default: us]
    --pretty                         Pretty-print JSON output
    -v, --verbose                    Verbose logging
    -h, --help                       Print help
```

**Examples:**
```bash
# Export Apple Books only
bookexport --apple-books-only --pretty -o library.json

# Export with Kindle clippings from device
bookexport --kindle-clippings "/Volumes/Kindle/documents/My Clippings.txt"

# Export with Amazon web scraping
bookexport --kindle-cookies cookies.txt --kindle-region us

# Full export with all sources
bookexport --kindle-clippings clippings.txt --kindle-cookies cookies.txt --pretty
```

## Configuration File

Optional TOML config at `~/.config/bookexport/config.toml`:

```toml
output_path = "~/.local/share/bookexport/library.json"

[apple_books]
enabled = true
# library_db = "..."      # Override default path
# annotation_db = "..."   # Override default path

[kindle]
enabled = true
region = "us"
# clippings_path = "..."  # Path to My Clippings.txt
# cookies_path = "..."    # Path to Amazon cookies file
```

## Deduplication Logic

1. **Book ID Generation:** `SHA256(lowercase(strip(title) + strip(author)))[:16]`
2. **Book Merging:** When same book found in multiple sources:
   - Combine `sources` vec
   - Merge highlights, dedupe by normalized text content
   - For `finished` status, `true` from any source wins
   - Prefer earlier `finished_at` date
3. **Highlight Deduplication:** Normalize text (lowercase, collapse whitespace) and compare

## Dependencies

Key crates:
- `rusqlite` (bundled) - SQLite database access
- `serde`, `serde_json` - JSON serialization
- `chrono` - Timestamp handling
- `clap` (derive) - CLI argument parsing
- `reqwest` (blocking, cookies) - HTTP requests for web scraping
- `scraper` - HTML parsing with CSS selectors
- `regex` - Text parsing
- `sha2` - Book ID generation
- `glob` - Finding database files
- `toml` - Config file parsing

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run directly
cargo run -- --help
```

## Testing

```bash
# Run tests
cargo test

# Test with your Apple Books
./target/release/bookexport --verbose --pretty --apple-books-only -o /tmp/test.json
```

## Known Limitations

1. **Kindle macOS App:** The local database only stores position markers, not highlight text. Use My Clippings.txt or web scraping instead.

2. **Amazon Copyright Limits:** Amazon truncates highlights after 10-20% of a book's content is highlighted.

3. **Web Scraping Fragility:** Amazon may show CAPTCHAs or rate-limit automated access. Cookies expire and need re-export.

4. **My Clippings.txt Language:** The parser assumes English-language format. Different Kindle language settings produce different date formats.

## Future Improvements

- Incremental sync (track `last_export_at` and filter by `created_at`)
- Support for more sources (Kobo, Google Play Books)
- Better date parsing for international My Clippings.txt formats
- Export to other formats (Markdown, CSV)
