use bookexport::{
    apple_books, kindle,
    model::{Library, Source},
    Config, Error,
};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

/// Export reading highlights from Apple Books and Kindle
#[derive(Parser, Debug)]
#[command(name = "bookexport")]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output path for the library JSON file
    #[arg(short, long, global = true)]
    output: Option<PathBuf>,

    /// Pretty-print JSON output
    #[arg(long, global = true)]
    pretty: bool,

    /// Verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Sync highlights from Kindle via browser (recommended)
    #[command(name = "kindle")]
    KindleSync {
        /// Amazon region: us, uk, de, fr, jp, etc.
        #[arg(long, default_value = "us")]
        region: String,

        /// Run browser in headless mode (no visible window)
        #[arg(long)]
        headless: bool,
    },

    /// Export from Apple Books only
    #[command(name = "apple-books")]
    AppleBooks,

    /// Legacy: use My Clippings.txt file from Kindle device
    #[command(name = "clippings")]
    Clippings {
        /// Path to My Clippings.txt file
        path: PathBuf,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let args = Args::parse();

    // Load config
    let config = Config::load_default();

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bookexport")
            .join("library.json")
    });

    if args.verbose {
        eprintln!("Output path: {}", output_path.display());
    }

    // Handle commands
    let books = match args.command {
        Some(Commands::KindleSync { region, headless }) => {
            run_kindle_browser_sync(&region, headless, args.verbose)?
        }
        Some(Commands::AppleBooks) => {
            run_apple_books_export(&config, args.verbose)?
        }
        Some(Commands::Clippings { path }) => {
            run_clippings_import(&path, args.verbose)?
        }
        None => {
            // Default: run Kindle browser sync
            eprintln!("No command specified. Running Kindle sync...");
            eprintln!("(Use --help to see all options)\n");
            run_kindle_browser_sync("us", false, args.verbose)?
        }
    };

    // Create library
    let library = Library {
        exported_at: Utc::now(),
        books,
    };

    // Summary
    let total_highlights: usize = library.books.iter().map(|b| b.highlights.len()).sum();
    let kindle_count = library
        .books
        .iter()
        .filter(|b| b.sources.contains(&Source::Kindle))
        .count();
    let apple_count = library
        .books
        .iter()
        .filter(|b| b.sources.contains(&Source::AppleBooks))
        .count();

    eprintln!(
        "\nExported {} books ({} Kindle, {} Apple Books) with {} total highlights",
        library.books.len(),
        kindle_count,
        apple_count,
        total_highlights
    );

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write output
    let json = if args.pretty {
        serde_json::to_string_pretty(&library)?
    } else {
        serde_json::to_string(&library)?
    };

    fs::write(&output_path, json)?;

    eprintln!("Written to {}", output_path.display());

    Ok(())
}

/// Run Kindle browser-based sync
fn run_kindle_browser_sync(region: &str, headless: bool, verbose: bool) -> Result<Vec<bookexport::Book>, Error> {
    eprintln!("Starting Kindle sync via browser...");

    let region = kindle::AmazonRegion::from_code(region).map_err(Error::Kindle)?;

    let config = kindle::BrowserConfig {
        headless,
        region,
        user_data_dir: None, // Will use default with session persistence
        timeout_secs: 30,
    };

    let scraper = kindle::KindleBrowserScraper::with_session_persistence(config)
        .map_err(|e| Error::Kindle(e))?;

    let books = scraper.scrape_all().map_err(Error::Kindle)?;

    if verbose {
        let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
        eprintln!("Found {} books with {} highlights", books.len(), highlight_count);
    }

    Ok(books)
}

/// Run Apple Books export
fn run_apple_books_export(config: &Config, verbose: bool) -> Result<Vec<bookexport::Book>, Error> {
    if verbose {
        eprintln!("Extracting from Apple Books...");
    }

    let books = apple_books::extract_full(
        config.apple_books.library_db.clone(),
        config.apple_books.annotation_db.clone(),
    ).map_err(Error::AppleBooks)?;

    if verbose {
        let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
        eprintln!("Found {} books with {} highlights", books.len(), highlight_count);
    }

    Ok(books)
}

/// Run My Clippings.txt import
fn run_clippings_import(path: &PathBuf, verbose: bool) -> Result<Vec<bookexport::Book>, Error> {
    if verbose {
        eprintln!("Parsing Kindle clippings from {}...", path.display());
    }

    let books = kindle::parse_clippings(path).map_err(Error::Kindle)?;

    if verbose {
        let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
        eprintln!("Found {} books with {} highlights", books.len(), highlight_count);
    }

    Ok(books)
}
