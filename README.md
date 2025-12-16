# readingsync

A Rust CLI tool to export reading highlights from Kindle and Apple Books on macOS.

## Features

- **Kindle Browser Sync** - Scrapes highlights from Amazon's Kindle Notebook via headless Chrome
- **Apple Books Export** - Extracts highlights from local SQLite databases
- **Kindle Clippings Import** - Parses `My Clippings.txt` from physical Kindle devices
- **Unified JSON Output** - All highlights merged and deduplicated into a single file

## Installation

### From Source

```bash
git clone https://github.com/urcades/readingsync.git
cd readingsync
cargo build --release
```

The binary will be at `./target/release/readingsync`.

### Requirements

- macOS (for Apple Books support)
- Rust toolchain
- Chrome/Chromium (for Kindle browser sync)

## Usage

```
readingsync [OPTIONS] [COMMAND]

Commands:
  kindle       Sync highlights from Kindle via browser (recommended)
  apple-books  Export from Apple Books only
  clippings    Import from Kindle's My Clippings.txt file
  help         Print help for a command

Options:
  -o, --output <PATH>  Output path [default: ~/.local/share/readingsync/library.json]
      --pretty         Pretty-print JSON output
  -v, --verbose        Show detailed progress
  -h, --help           Print help
  -V, --version        Print version
```

## Commands

### `kindle` - Browser-based Kindle Sync (Recommended)

Scrapes highlights directly from Amazon's Kindle Notebook website using browser automation.

```bash
# First run - opens browser for login
readingsync kindle --region us --verbose

# Subsequent runs - headless mode (no browser window)
readingsync kindle --region us --headless --verbose
```

**Options:**
- `--region <REGION>` - Amazon region (default: `us`)
  - Supported: `us`, `uk`, `de`, `fr`, `es`, `it`, `jp`, `ca`, `au`, `in`
- `--headless` - Run browser in background (use after first login)

**How it works:**
1. First run opens a Chrome window and navigates to `read.amazon.com/notebook`
2. You log in to your Amazon account (session is saved for future runs)
3. The tool scrapes all books and highlights from your library
4. Subsequent runs can use `--headless` since you're already authenticated

### `apple-books` - Apple Books Export

Extracts highlights from the local Apple Books databases on macOS.

```bash
readingsync apple-books --verbose --pretty
```

**Database locations:**
- Library: `~/Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary/`
- Annotations: `~/Library/Containers/com.apple.iBooksX/Data/Documents/AEAnnotation/`

### `clippings` - Kindle Device Import

Parses the `My Clippings.txt` file from a physical Kindle device.

```bash
# From mounted Kindle
readingsync clippings "/Volumes/Kindle/documents/My Clippings.txt"

# From copied file
readingsync clippings ~/Downloads/My\ Clippings.txt
```

## Output Format

All commands output JSON in this format:

```json
{
  "exported_at": "2025-12-15T16:06:47.321267Z",
  "books": [
    {
      "id": "c80c567945e10470",
      "title": "Steve Jobs",
      "author": "Walter Isaacson",
      "sources": ["kindle"],
      "highlights": [
        {
          "id": "585499d4-8a40-43c5-a6ef-53979f6d012a",
          "text": "The highlighted text...",
          "note": null,
          "location": {
            "chapter": null,
            "position": "Location 123"
          },
          "created_at": null,
          "source": "kindle"
        }
      ],
      "finished": null,
      "finished_at": null
    }
  ]
}
```

## Examples

```bash
# Sync Kindle highlights (first time - visible browser)
readingsync kindle --region us --verbose --pretty -o highlights.json

# Sync Kindle highlights (after login - background)
readingsync kindle --region us --headless --pretty

# Export Apple Books only
readingsync apple-books --pretty -o apple-highlights.json

# Import from Kindle device
readingsync clippings /Volumes/Kindle/documents/My\ Clippings.txt --pretty

# Default behavior (runs Kindle sync)
readingsync --verbose
```

## Configuration

An optional TOML config file can be placed at `~/.config/readingsync/config.toml`:

```toml
output_path = "~/.local/share/readingsync/library.json"

[apple_books]
enabled = true
# library_db = "..."      # Override default path
# annotation_db = "..."   # Override default path

[kindle]
enabled = true
region = "us"
```

## How It Works

### Kindle Browser Sync

The tool uses the `headless_chrome` crate to automate a real Chrome browser:

1. Launches Chrome with a persistent profile (saves login session)
2. Navigates to `read.amazon.com/notebook`
3. Waits for you to log in (first run only)
4. Extracts the book list from the sidebar
5. Clicks each book and scrapes its highlights
6. Outputs unified JSON

Session data is stored in `~/.local/share/readingsync/chrome_profile/`.

### Apple Books

Reads directly from Apple Books' SQLite databases:
- `BKLibrary*.sqlite` for book metadata
- `AEAnnotation*.sqlite` for highlights

The databases are copied to a temp location before reading to avoid lock conflicts.

### Deduplication

Books are identified by `SHA256(lowercase(title + author))[:16]`. When the same book appears in multiple sources:
- Highlights are merged and deduplicated by text content
- Sources are combined (e.g., `["kindle", "apple_books"]`)

## Known Limitations

1. **Amazon Rate Limiting** - Amazon may temporarily block access if you scrape too frequently
2. **Session Expiry** - Amazon sessions expire after a few weeks; run without `--headless` to re-authenticate
3. **Copyright Limits** - Amazon truncates highlights after 10-20% of a book's content
4. **macOS Only** - Apple Books extraction only works on macOS

## Development

```bash
# Run tests
cargo test

# Build debug
cargo build

# Build release
cargo build --release

# Run with arguments
cargo run -- kindle --region us --verbose
```

## License

MIT
