# CLAUDE.md - Project Overview

## What is this project?

`readingsync` is a Rust CLI tool that extracts reading highlights from Kindle and Apple Books on macOS, merges/deduplicates books across sources, and outputs a unified JSON file.

## Project Structure

```
readingsync/
├── Cargo.toml              # Dependencies and project metadata
├── Cargo.lock              # Locked dependency versions
├── README.md               # User documentation
├── CLAUDE.md               # This file (development reference)
└── src/
    ├── main.rs             # CLI entry point with subcommands
    ├── lib.rs              # Library re-exports
    ├── model.rs            # Data structures (Library, Book, Highlight, Source, Location)
    ├── error.rs            # Error types (AppleBooksError, KindleError, ConfigError)
    ├── apple_books.rs      # Apple Books SQLite extraction
    ├── kindle/
    │   ├── mod.rs          # Kindle module exports
    │   ├── browser.rs      # Headless Chrome browser scraper (primary method)
    │   ├── clippings.rs    # My Clippings.txt parser
    │   └── scraper.rs      # Legacy cookie-based web scraper
    ├── merge.rs            # Book/highlight deduplication logic
    └── config.rs           # TOML config file support
```

## CLI Commands

```bash
# Primary: Browser-based Kindle sync
readingsync kindle --region us [--headless] [--verbose]

# Apple Books export
readingsync apple-books [--verbose]

# Kindle device clippings import
readingsync clippings <PATH> [--verbose]
```

Global flags: `-o/--output`, `--pretty`, `-v/--verbose`

## Data Model

```rust
struct Library {
    exported_at: DateTime<Utc>,
    books: Vec<Book>,
}

struct Book {
    id: String,                    // SHA256(lowercase(title + author))[:16]
    title: String,
    author: Option<String>,
    sources: Vec<Source>,          // Which platforms this book was found on
    highlights: Vec<Highlight>,
    finished: Option<bool>,
    finished_at: Option<DateTime<Utc>>,
}

struct Highlight {
    id: String,                    // UUID
    text: String,
    note: Option<String>,
    location: Location,
    created_at: Option<DateTime<Utc>>,
    source: Source,
}

struct Location {
    chapter: Option<String>,
    position: Option<String>,      // e.g., "Location 123"
}

enum Source {
    AppleBooks,
    Kindle,
}
```

## Data Sources

### Kindle - Browser Scraper (Primary Method)

**File:** `src/kindle/browser.rs`

Uses `headless_chrome` crate to automate a real Chrome browser:

1. Launches Chrome with persistent profile at `~/.local/share/readingsync/chrome_profile/`
2. Navigates to `read.amazon.com/notebook`
3. First run: waits for user to log in via visible browser window
4. Subsequent runs: can use `--headless` flag for background operation
5. Extracts book list from sidebar via JavaScript
6. Clicks each book using native Chrome DevTools Protocol click
7. Waits for content to change (detects by comparing first highlight text)
8. Scrapes highlights via JavaScript DOM queries

**Key Components:**
- `AmazonRegion` - Region-specific URLs (us, uk, de, fr, es, it, jp, ca, au, in)
- `BrowserConfig` - Headless mode, region, timeout, user data dir
- `KindleBrowserScraper` - Main scraper with session persistence

**CSS Selectors:**
- Book list: `.kp-notebook-library-each-book` (id attribute = ASIN)
- Book title: `h2.kp-notebook-searchable`
- Book author: `p.kp-notebook-searchable`
- Highlight text: `#highlight`
- Note: `#note`
- Location: `#kp-annotation-location`

**Why browser automation?**
- Amazon's Kindle Notebook is a React SPA that requires JavaScript
- Cookie-based scraping failed with HTTP 400 errors on book pages
- Native Chrome clicks properly trigger React event handlers
- Session persistence means login only needed once

### Apple Books (macOS)

**File:** `src/apple_books.rs`

**Database Locations:**
- Library: `~/Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary/BKLibrary*.sqlite`
- Annotations: `~/Library/Containers/com.apple.iBooksX/Data/Documents/AEAnnotation/AEAnnotation*.sqlite`

**Key Tables:**
- `ZBKLIBRARYASSET` - Book metadata (title, author, finished status)
- `ZAEANNOTATION` - Highlights and notes

**Important Notes:**
- Timestamps are CoreData format (seconds since 2001-01-01). Convert: `timestamp + 978307200` → Unix epoch
- Books and annotations linked by `ZASSETID` ↔ `ZANNOTATIONASSETID`
- Databases copied to temp location before reading to avoid SQLITE_BUSY

### Kindle - My Clippings.txt

**File:** `src/kindle/clippings.rs`

**Location:** `/documents/My Clippings.txt` on Kindle device

**Format:**
```
Book Title (Author Name)
- Your Highlight on Location 123-145 | Added on Monday, January 1, 2024

The actual highlighted text goes here...
==========
```

**Parsing:** Split by `==========`, extract title/author via regex, parse location from metadata line.

### Kindle - Legacy Cookie Scraper

**File:** `src/kindle/scraper.rs`

Cookie-based HTTP scraper (renamed to `LegacyAmazonRegion` to avoid conflicts). Not recommended - Amazon blocks direct URL navigation to book pages.

## Deduplication Logic

**File:** `src/merge.rs`

1. **Book ID:** `SHA256(lowercase(strip(title) + strip(author)))[:16]`
2. **Book Merging:** Combine sources, merge highlights, dedupe by normalized text
3. **Highlight Deduplication:** Normalize (lowercase, collapse whitespace), compare

## Dependencies

Key crates:
- `headless_chrome` - Browser automation via Chrome DevTools Protocol
- `rusqlite` (bundled) - SQLite database access
- `serde`, `serde_json` - JSON serialization
- `chrono` - Timestamp handling
- `clap` (derive) - CLI argument parsing
- `reqwest` (blocking, cookies) - HTTP requests (legacy scraper)
- `scraper` - HTML parsing with CSS selectors
- `regex` - Text parsing
- `sha2` - Book ID generation
- `uuid` - Highlight ID generation
- `dirs` - Platform-specific directories

## Building & Testing

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run tests
cargo run -- --help      # Run with args
```

## Session Persistence

Browser sessions stored at: `~/.local/share/readingsync/chrome_profile/`

After first login, use `--headless` for background operation. Sessions expire after ~2-4 weeks.

## Known Limitations

1. **Kindle macOS App:** Local database only stores position markers, not highlight text
2. **Amazon Copyright Limits:** Highlights truncated after 10-20% of book content
3. **Session Expiry:** Amazon sessions expire; re-run without `--headless` to re-authenticate
4. **macOS Only:** Apple Books extraction requires macOS

## Test Results

Last test run: **84 books, 2,686 highlights** from Kindle via browser scraper.
