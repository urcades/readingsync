use bookexport::{
    apple_books, kindle, merge,
    model::{Library, Source},
    Config, Error,
};
use chrono::Utc;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// Export reading highlights from Apple Books and Kindle
#[derive(Parser, Debug)]
#[command(name = "bookexport")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output path for the library JSON file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Config file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Only export from Apple Books
    #[arg(long)]
    apple_books_only: bool,

    /// Path to Kindle's My Clippings.txt file
    #[arg(long)]
    kindle_clippings: Option<PathBuf>,

    /// Path to exported Amazon cookies file (Netscape format)
    #[arg(long)]
    kindle_cookies: Option<PathBuf>,

    /// Amazon region: us, uk, de, fr, jp, etc.
    #[arg(long, default_value = "us")]
    kindle_region: String,

    /// Pretty-print JSON output
    #[arg(long)]
    pretty: bool,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
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
    let mut config = if let Some(config_path) = &args.config {
        Config::load(config_path).map_err(Error::Config)?
    } else {
        Config::load_default()
    };
    config.expand_paths();

    // Determine output path
    let output_path = args.output.unwrap_or(config.output_path.clone());

    if args.verbose {
        eprintln!("Output path: {}", output_path.display());
    }

    // Collect books from all sources
    let mut book_lists = Vec::new();

    // Apple Books
    if config.apple_books.enabled && !args.kindle_clippings.is_some() || !args.apple_books_only {
        if args.verbose {
            eprintln!("Extracting from Apple Books...");
        }

        match apple_books::extract_full(
            config.apple_books.library_db.clone(),
            config.apple_books.annotation_db.clone(),
        ) {
            Ok(books) => {
                if args.verbose {
                    let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
                    eprintln!(
                        "  Found {} books with {} highlights",
                        books.len(),
                        highlight_count
                    );
                }
                book_lists.push(books);
            }
            Err(e) => {
                if args.verbose {
                    eprintln!("  Warning: Failed to extract from Apple Books: {}", e);
                }
            }
        }
    }

    // Kindle - My Clippings.txt
    if !args.apple_books_only {
        let clippings_path = args
            .kindle_clippings
            .or(config.kindle.clippings_path.clone());

        if let Some(path) = clippings_path {
            if args.verbose {
                eprintln!("Parsing Kindle clippings from {}...", path.display());
            }

            match kindle::parse_clippings(&path) {
                Ok(books) => {
                    if args.verbose {
                        let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
                        eprintln!(
                            "  Found {} books with {} highlights",
                            books.len(),
                            highlight_count
                        );
                    }
                    book_lists.push(books);
                }
                Err(e) => {
                    if args.verbose {
                        eprintln!("  Warning: Failed to parse clippings: {}", e);
                    }
                }
            }
        }
    }

    // Kindle - Amazon Notebook scraping
    if !args.apple_books_only {
        let cookies_path = args.kindle_cookies.or(config.kindle.cookies_path.clone());

        if let Some(path) = cookies_path {
            if args.verbose {
                eprintln!("Scraping Amazon Notebook...");
            }

            let region = kindle::AmazonRegion::from_code(&args.kindle_region)
                .or_else(|_| kindle::AmazonRegion::from_code(&config.kindle.region))
                .map_err(Error::Kindle)?;

            match kindle::scrape_highlights(&path, &region) {
                Ok(books) => {
                    if args.verbose {
                        let highlight_count: usize = books.iter().map(|b| b.highlights.len()).sum();
                        eprintln!(
                            "  Found {} books with {} highlights",
                            books.len(),
                            highlight_count
                        );
                    }
                    book_lists.push(books);
                }
                Err(e) => {
                    if args.verbose {
                        eprintln!("  Warning: Failed to scrape Amazon Notebook: {}", e);
                    }
                }
            }
        }
    }

    // Merge books from all sources
    if args.verbose {
        eprintln!("Merging books from {} sources...", book_lists.len());
    }

    let books = merge::merge_books(book_lists);

    // Create library
    let library = Library {
        exported_at: Utc::now(),
        books,
    };

    // Summary
    let total_highlights: usize = library.books.iter().map(|b| b.highlights.len()).sum();
    let apple_books_count = library
        .books
        .iter()
        .filter(|b| b.sources.contains(&Source::AppleBooks))
        .count();
    let kindle_count = library
        .books
        .iter()
        .filter(|b| b.sources.contains(&Source::Kindle))
        .count();

    eprintln!(
        "Exported {} books ({} from Apple Books, {} from Kindle) with {} total highlights",
        library.books.len(),
        apple_books_count,
        kindle_count,
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
