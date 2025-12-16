use crate::error::KindleError;
use crate::model::{generate_book_id, Book, Highlight, Location, Source};
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Amazon region configuration for browser-based scraping
#[derive(Debug, Clone)]
pub struct AmazonRegion {
    pub code: String,
    pub notebook_url: String,
    pub signin_url: String,
}

impl AmazonRegion {
    pub fn from_code(code: &str) -> Result<Self, KindleError> {
        let (notebook_url, signin_url) = match code.to_lowercase().as_str() {
            "us" => (
                "https://read.amazon.com/notebook",
                "https://www.amazon.com/ap/signin",
            ),
            "uk" | "gb" => (
                "https://read.amazon.co.uk/notebook",
                "https://www.amazon.co.uk/ap/signin",
            ),
            "de" => (
                "https://read.amazon.de/notebook",
                "https://www.amazon.de/ap/signin",
            ),
            "fr" => (
                "https://read.amazon.fr/notebook",
                "https://www.amazon.fr/ap/signin",
            ),
            "es" => (
                "https://read.amazon.es/notebook",
                "https://www.amazon.es/ap/signin",
            ),
            "it" => (
                "https://read.amazon.it/notebook",
                "https://www.amazon.it/ap/signin",
            ),
            "jp" => (
                "https://read.amazon.co.jp/notebook",
                "https://www.amazon.co.jp/ap/signin",
            ),
            "ca" => (
                "https://read.amazon.ca/notebook",
                "https://www.amazon.ca/ap/signin",
            ),
            "au" => (
                "https://read.amazon.com.au/notebook",
                "https://www.amazon.com.au/ap/signin",
            ),
            "in" => (
                "https://read.amazon.in/notebook",
                "https://www.amazon.in/ap/signin",
            ),
            _ => return Err(KindleError::InvalidRegion(code.to_string())),
        };

        Ok(Self {
            code: code.to_lowercase(),
            notebook_url: notebook_url.to_string(),
            signin_url: signin_url.to_string(),
        })
    }
}

/// Configuration for the browser scraper
pub struct BrowserConfig {
    /// Whether to run in headless mode (false = show browser window)
    pub headless: bool,
    /// Amazon region
    pub region: AmazonRegion,
    /// Path to store user data (for session persistence)
    pub user_data_dir: Option<String>,
    /// Timeout for page loads in seconds
    pub timeout_secs: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: false, // Show browser by default for login
            region: AmazonRegion::from_code("us").unwrap(),
            user_data_dir: None,
            timeout_secs: 30,
        }
    }
}

/// Scrape Kindle highlights using a headless browser
pub struct KindleBrowserScraper {
    browser: Browser,
    config: BrowserConfig,
}

impl KindleBrowserScraper {
    /// Create a new browser scraper
    pub fn new(config: BrowserConfig) -> Result<Self, KindleError> {
        let mut launch_options = LaunchOptions::default_builder();

        launch_options
            .headless(config.headless)
            .window_size(Some((1280, 900)));

        // Set user data directory for session persistence
        if let Some(ref user_data_dir) = config.user_data_dir {
            launch_options.user_data_dir(Some(std::path::PathBuf::from(user_data_dir)));
        }

        let launch_options = launch_options
            .build()
            .map_err(|e| KindleError::ParseError(format!("Failed to build launch options: {}", e)))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| KindleError::ParseError(format!("Failed to launch browser: {}", e)))?;

        Ok(Self { browser, config })
    }

    /// Create with default user data directory for session persistence
    pub fn with_session_persistence(mut config: BrowserConfig) -> Result<Self, KindleError> {
        if config.user_data_dir.is_none() {
            let data_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("readingsync")
                .join("chrome_profile");

            // Create directory if it doesn't exist
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| KindleError::ParseError(format!("Failed to create profile dir: {}", e)))?;

            config.user_data_dir = Some(data_dir.to_string_lossy().to_string());
        }

        Self::new(config)
    }

    /// Get a new tab
    fn new_tab(&self) -> Result<Arc<Tab>, KindleError> {
        self.browser
            .new_tab()
            .map_err(|e| KindleError::ParseError(format!("Failed to create tab: {}", e)))
    }

    /// Wait for user to complete login
    pub fn wait_for_login(&self, tab: &Tab) -> Result<(), KindleError> {
        eprintln!("Navigating to Amazon Kindle notebook...");

        tab.navigate_to(&self.config.region.notebook_url)
            .map_err(|e| KindleError::ParseError(format!("Failed to navigate: {}", e)))?;

        // Wait for page to load
        thread::sleep(Duration::from_secs(2));

        // Check if we need to log in
        let url = tab.get_url();
        if url.contains("signin") || url.contains("ap/signin") {
            eprintln!("\n╔════════════════════════════════════════════════════════════╗");
            eprintln!("║  Please log in to your Amazon account in the browser window ║");
            eprintln!("║  Press Enter here once you've completed login...            ║");
            eprintln!("╚════════════════════════════════════════════════════════════╝\n");

            // Wait for user input
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)
                .map_err(|e| KindleError::ParseError(format!("Failed to read input: {}", e)))?;
        }

        // Wait for notebook page to load
        self.wait_for_notebook_page(tab)?;

        eprintln!("Successfully logged in!");
        Ok(())
    }

    /// Wait for the notebook page to be fully loaded
    fn wait_for_notebook_page(&self, tab: &Tab) -> Result<(), KindleError> {
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(KindleError::ParseError("Timeout waiting for notebook page".to_string()));
            }

            let url = tab.get_url();
            if url.contains("notebook") && !url.contains("signin") {
                // Try to find the book list element
                if tab.find_element(".kp-notebook-library-each-book").is_ok() {
                    return Ok(());
                }
                // Also check for empty library message
                if tab.find_element("#kp-notebook-library").is_ok() {
                    return Ok(());
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
    }

    /// Scrape all books and highlights
    pub fn scrape_all(&self) -> Result<Vec<Book>, KindleError> {
        let tab = self.new_tab()?;

        // Ensure we're logged in
        self.wait_for_login(&tab)?;

        // Get list of books
        eprintln!("Fetching book list...");
        let book_asins = self.get_book_list(&tab)?;
        eprintln!("Found {} books", book_asins.len());

        let mut books = Vec::new();

        for (i, (asin, title, author)) in book_asins.iter().enumerate() {
            eprintln!("  [{}/{}] Scraping: {}", i + 1, book_asins.len(), title);

            match self.scrape_book_highlights(&tab, asin, title, author.as_deref()) {
                Ok(book) => {
                    eprintln!("    → {} highlights", book.highlights.len());
                    books.push(book);
                }
                Err(e) => {
                    eprintln!("    → Error: {}", e);
                }
            }

            // Small delay between books to avoid rate limiting
            thread::sleep(Duration::from_millis(500));
        }

        Ok(books)
    }

    /// Get list of books from the notebook page
    fn get_book_list(&self, tab: &Tab) -> Result<Vec<(String, String, Option<String>)>, KindleError> {
        // Navigate to notebook if not already there
        let url = tab.get_url();
        if !url.contains("notebook") {
            tab.navigate_to(&self.config.region.notebook_url)
                .map_err(|e| KindleError::ParseError(format!("Failed to navigate: {}", e)))?;
            self.wait_for_notebook_page(tab)?;
        }

        // Execute JavaScript to extract book data
        let js = r#"
            (function() {
                const books = [];
                const elements = document.querySelectorAll('.kp-notebook-library-each-book');
                elements.forEach(el => {
                    const asin = el.id || '';
                    const titleEl = el.querySelector('h2');
                    const authorEl = el.querySelector('p.kp-notebook-searchable');

                    const title = titleEl ? titleEl.textContent.trim() : '';
                    let author = authorEl ? authorEl.textContent.trim() : '';

                    // Remove "By: " prefix
                    if (author.toLowerCase().startsWith('by:')) {
                        author = author.substring(3).trim();
                    }

                    if (asin && title) {
                        books.push({asin: asin, title: title, author: author || null});
                    }
                });
                return JSON.stringify(books);
            })()
        "#;

        let result = tab.evaluate(js, true)
            .map_err(|e| KindleError::ParseError(format!("Failed to execute JS: {}", e)))?;

        let json_str = result
            .value
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| KindleError::ParseError("Failed to get book list".to_string()))?;

        let book_data: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .map_err(|e| KindleError::ParseError(format!("Failed to parse book list: {}", e)))?;

        let books = book_data
            .into_iter()
            .filter_map(|v| {
                let asin = v.get("asin")?.as_str()?.to_string();
                let title = v.get("title")?.as_str()?.to_string();
                let author = v.get("author").and_then(|a| a.as_str()).map(String::from);
                Some((asin, title, author))
            })
            .collect();

        Ok(books)
    }

    /// Scrape highlights for a specific book
    fn scrape_book_highlights(
        &self,
        tab: &Tab,
        asin: &str,
        title: &str,
        author: Option<&str>,
    ) -> Result<Book, KindleError> {
        // Get the current first highlight text before clicking (to detect change)
        let get_first_highlight_js = r#"
            (function() {
                const el = document.querySelector('#highlight');
                return el ? el.textContent.trim().substring(0, 50) : '';
            })()
        "#;

        let old_highlight = tab.evaluate(get_first_highlight_js, true)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        // Click on the book in the sidebar using native click
        let selector = format!("#{}", asin);
        let element = tab.find_element(&selector)
            .map_err(|e| KindleError::ParseError(format!("Could not find book element {}: {}", asin, e)))?;

        // Scroll into view first
        element.scroll_into_view()
            .map_err(|e| KindleError::ParseError(format!("Failed to scroll: {}", e)))?;

        thread::sleep(Duration::from_millis(200));

        // Click using headless_chrome native click
        element.click()
            .map_err(|e| KindleError::ParseError(format!("Failed to click: {}", e)))?;

        // Wait for content to change (either different highlight or loading state)
        let timeout = Duration::from_secs(10);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                break;
            }

            let new_highlight = tab.evaluate(get_first_highlight_js, true)
                .ok()
                .and_then(|r| r.value)
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();

            // Content changed or cleared (loading)
            if new_highlight != old_highlight || new_highlight.is_empty() {
                // If empty, wait a bit more for content to load
                if new_highlight.is_empty() {
                    thread::sleep(Duration::from_millis(500));
                }
                break;
            }

            thread::sleep(Duration::from_millis(100));
        }

        // Extra delay to ensure DOM is fully updated
        thread::sleep(Duration::from_secs(1));

        // Collect all highlights with pagination
        let mut all_highlights = Vec::new();
        let mut page = 1;

        loop {
            let (highlights, has_more) = self.extract_highlights_from_page(tab)?;
            all_highlights.extend(highlights);

            if !has_more {
                break;
            }

            // Click "next page" and wait
            page += 1;
            if page > 100 {
                // Safety limit
                break;
            }

            if !self.click_next_page(tab)? {
                break;
            }

            thread::sleep(Duration::from_secs(1));
        }

        let id = generate_book_id(title, author);
        Ok(Book {
            id,
            title: title.to_string(),
            author: author.map(String::from),
            sources: vec![Source::Kindle],
            highlights: all_highlights,
            finished: None,
            finished_at: None,
        })
    }

    /// Extract highlights from the current page
    fn extract_highlights_from_page(&self, tab: &Tab) -> Result<(Vec<Highlight>, bool), KindleError> {
        let js = r#"
            (function() {
                const highlights = [];
                const seen = new Set();

                // Find all highlight containers
                const containers = document.querySelectorAll('.a-row.a-spacing-base');

                containers.forEach(container => {
                    const highlightEl = container.querySelector('#highlight');
                    const noteEl = container.querySelector('#note');
                    const locationEl = container.querySelector('#kp-annotation-location');

                    if (highlightEl) {
                        const text = highlightEl.textContent.trim();
                        if (text && !seen.has(text)) {
                            seen.add(text);

                            const note = noteEl ? noteEl.textContent.trim() : null;
                            const location = locationEl ? locationEl.textContent.trim() : null;

                            // Try to get highlight color
                            let color = null;
                            const colorEl = container.querySelector('[class*="kp-notebook-highlight"]');
                            if (colorEl) {
                                const classes = colorEl.className;
                                const match = classes.match(/kp-notebook-highlight-(\w+)/);
                                if (match) color = match[1];
                            }

                            highlights.push({
                                text: text,
                                note: note || null,
                                location: location || null,
                                color: color
                            });
                        }
                    }
                });

                // Check for pagination
                const nextPageEl = document.querySelector('.kp-notebook-annotations-next-page-start');
                const hasMore = nextPageEl && nextPageEl.value && nextPageEl.value.length > 0;

                return JSON.stringify({highlights: highlights, hasMore: hasMore});
            })()
        "#;

        let result = tab.evaluate(js, true)
            .map_err(|e| KindleError::ParseError(format!("Failed to execute JS: {}", e)))?;

        let json_str = result
            .value
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| KindleError::ParseError("Failed to get highlights".to_string()))?;

        let data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| KindleError::ParseError(format!("Failed to parse highlights: {}", e)))?;

        let has_more = data.get("hasMore").and_then(|v| v.as_bool()).unwrap_or(false);

        let highlights = data
            .get("highlights")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let text = v.get("text")?.as_str()?.to_string();
                        let note = v.get("note").and_then(|n| n.as_str()).map(String::from);
                        let position = v.get("location").and_then(|l| l.as_str()).map(String::from);

                        Some(Highlight {
                            id: uuid::Uuid::new_v4().to_string(),
                            text,
                            note,
                            location: Location {
                                chapter: None,
                                position,
                            },
                            created_at: None,
                            source: Source::Kindle,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok((highlights, has_more))
    }

    /// Click the "next page" button for pagination
    fn click_next_page(&self, tab: &Tab) -> Result<bool, KindleError> {
        let js = r#"
            (function() {
                // Find the "next page" link/button
                const nextBtn = document.querySelector('.kp-notebook-annotations-paging a[href*="token"]');
                if (nextBtn) {
                    nextBtn.click();
                    return true;
                }

                // Alternative: look for a form submit
                const nextPageInput = document.querySelector('.kp-notebook-annotations-next-page-start');
                if (nextPageInput && nextPageInput.value) {
                    // Trigger form submission or navigation
                    const form = nextPageInput.closest('form');
                    if (form) {
                        form.submit();
                        return true;
                    }
                }

                return false;
            })()
        "#;

        let result = tab.evaluate(js, true)
            .map_err(|e| KindleError::ParseError(format!("Failed to click next: {}", e)))?;

        Ok(result.value.and_then(|v| v.as_bool()).unwrap_or(false))
    }
}
