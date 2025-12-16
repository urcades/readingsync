use crate::error::KindleError;
use crate::model::{generate_book_id, Book, Highlight, Location, Source};
use reqwest::blocking::Client;
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use url::Url;

/// Amazon region configuration for cookie-based scraping (legacy)
#[derive(Debug, Clone)]
pub struct LegacyAmazonRegion {
    pub code: String,
    pub domain: String,
    pub notebook_url: String,
}

impl LegacyAmazonRegion {
    pub fn from_code(code: &str) -> Result<Self, KindleError> {
        let (domain, notebook_url) = match code.to_lowercase().as_str() {
            "us" => ("amazon.com", "https://read.amazon.com/notebook"),
            "uk" | "gb" => ("amazon.co.uk", "https://read.amazon.co.uk/notebook"),
            "de" => ("amazon.de", "https://read.amazon.de/notebook"),
            "fr" => ("amazon.fr", "https://read.amazon.fr/notebook"),
            "es" => ("amazon.es", "https://read.amazon.es/notebook"),
            "it" => ("amazon.it", "https://read.amazon.it/notebook"),
            "jp" => ("amazon.co.jp", "https://read.amazon.co.jp/notebook"),
            "ca" => ("amazon.ca", "https://read.amazon.ca/notebook"),
            "au" => ("amazon.com.au", "https://read.amazon.com.au/notebook"),
            "in" => ("amazon.in", "https://read.amazon.in/notebook"),
            "br" => ("amazon.com.br", "https://read.amazon.com.br/notebook"),
            "mx" => ("amazon.com.mx", "https://read.amazon.com.mx/notebook"),
            _ => return Err(KindleError::InvalidRegion(code.to_string())),
        };

        Ok(Self {
            code: code.to_lowercase(),
            domain: domain.to_string(),
            notebook_url: notebook_url.to_string(),
        })
    }
}

/// Scrape highlights from Amazon's Kindle Notebook (legacy cookie-based method)
pub fn scrape_highlights(
    cookies_path: &Path,
    region: &LegacyAmazonRegion,
) -> Result<Vec<Book>, KindleError> {
    if !cookies_path.exists() {
        return Err(KindleError::CookieFileNotFound(cookies_path.to_path_buf()));
    }

    // Load cookies
    let jar = load_cookies(cookies_path, &region.domain)?;

    // Create HTTP client with cookies
    let client = Client::builder()
        .cookie_provider(Arc::new(jar))
        .default_headers(default_headers())
        .build()?;

    // Fetch book list
    let books_data = fetch_book_list(&client, region)?;

    // Fetch highlights for each book
    let mut books = Vec::new();
    for book_data in books_data {
        let highlights = fetch_book_highlights(&client, region, &book_data.asin)?;

        let id = generate_book_id(&book_data.title, book_data.author.as_deref());
        let book = Book {
            id,
            title: book_data.title,
            author: book_data.author,
            sources: vec![Source::Kindle],
            highlights,
            finished: None,
            finished_at: None,
        };
        books.push(book);
    }

    Ok(books)
}

/// Default headers for requests
fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );
    headers
}

/// Load cookies from Netscape format file
fn load_cookies(path: &Path, domain: &str) -> Result<Jar, KindleError> {
    let content = fs::read_to_string(path)
        .map_err(|e| KindleError::CookieLoadError(format!("Failed to read cookie file: {}", e)))?;

    let jar = Jar::default();
    let base_url = format!("https://{}", domain)
        .parse::<Url>()
        .map_err(|e| KindleError::CookieLoadError(format!("Invalid URL: {}", e)))?;

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Netscape format: domain  flag  path  secure  expiration  name  value
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 7 {
            let cookie_domain = parts[0].trim_start_matches('.');
            let name = parts[5];
            let value = parts[6];

            // Only add cookies for the target domain
            if cookie_domain.contains(domain) || domain.contains(cookie_domain) {
                let cookie = format!("{}={}", name, value);
                jar.add_cookie_str(&cookie, &base_url);
            }
        }
    }

    Ok(jar)
}

#[derive(Debug)]
struct BookData {
    asin: String,
    title: String,
    author: Option<String>,
}

/// Fetch the list of books from the notebook page
fn fetch_book_list(client: &Client, region: &LegacyAmazonRegion) -> Result<Vec<BookData>, KindleError> {
    let response = client.get(&region.notebook_url).send()?;

    if !response.status().is_success() {
        return Err(KindleError::NotAuthenticated);
    }

    let html = response.text()?;

    // Check for login redirect
    if html.contains("ap_email") || html.contains("signIn") {
        return Err(KindleError::NotAuthenticated);
    }

    parse_book_list(&html)
}

/// Parse book list from HTML
fn parse_book_list(html: &str) -> Result<Vec<BookData>, KindleError> {
    let document = Html::parse_document(html);

    // Selector for book entries
    let book_selector = Selector::parse(".kp-notebook-library-each-book")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let title_selector = Selector::parse("h2.kp-notebook-searchable")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let author_selector = Selector::parse("p.kp-notebook-searchable")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let mut books = Vec::new();

    for book_elem in document.select(&book_selector) {
        // Get ASIN from id attribute
        let asin = book_elem.value().id().map(String::from).unwrap_or_default();

        if asin.is_empty() {
            continue;
        }

        // Get title
        let title = book_elem
            .select(&title_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Get author
        let author = book_elem.select(&author_selector).next().and_then(|e| {
            let text = e.text().collect::<String>();
            // Remove "By: " prefix if present
            let cleaned = text
                .trim()
                .trim_start_matches("By:")
                .trim_start_matches("by:")
                .trim();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned.to_string())
            }
        });

        books.push(BookData { asin, title, author });
    }

    Ok(books)
}

/// Fetch highlights for a specific book
fn fetch_book_highlights(
    client: &Client,
    region: &LegacyAmazonRegion,
    asin: &str,
) -> Result<Vec<Highlight>, KindleError> {
    let mut highlights = Vec::new();
    let mut pagination_token: Option<String> = None;
    let mut content_limit_state: Option<String> = None;

    loop {
        // Build URL with pagination params
        let mut url = format!("{}?asin={}", region.notebook_url, asin);
        if let Some(ref token) = pagination_token {
            url.push_str(&format!("&token={}", token));
        }
        if let Some(ref state) = content_limit_state {
            url.push_str(&format!("&contentLimitState={}", state));
        }

        let response = client.get(&url).send()?;
        let html = response.text()?;

        let (page_highlights, next_token, next_state) = parse_highlights_page(&html)?;
        highlights.extend(page_highlights);

        // Check for next page
        if next_token.is_some() {
            pagination_token = next_token;
            content_limit_state = next_state;
        } else {
            break;
        }
    }

    Ok(highlights)
}

/// Parse highlights from a single page
fn parse_highlights_page(
    html: &str,
) -> Result<(Vec<Highlight>, Option<String>, Option<String>), KindleError> {
    let document = Html::parse_document(html);

    // Selectors for highlights
    let highlight_container_selector = Selector::parse(".a-row.a-spacing-base")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let highlight_text_selector = Selector::parse("#highlight")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let note_selector = Selector::parse("#note")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let location_selector = Selector::parse("#kp-annotation-location")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    // Pagination selectors
    let next_page_selector = Selector::parse(".kp-notebook-annotations-next-page-start")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let content_limit_selector = Selector::parse(".kp-notebook-content-limit-state")
        .map_err(|e| KindleError::ParseError(format!("Invalid selector: {:?}", e)))?;

    let mut highlights = Vec::new();
    let mut seen_texts: std::collections::HashSet<String> = std::collections::HashSet::new();

    for container in document.select(&highlight_container_selector) {
        // Get highlight text
        let text = match container.select(&highlight_text_selector).next() {
            Some(elem) => elem.text().collect::<String>().trim().to_string(),
            None => continue,
        };

        if text.is_empty() {
            continue;
        }

        // Deduplicate by text
        if seen_texts.contains(&text) {
            continue;
        }
        seen_texts.insert(text.clone());

        // Get note if present
        let note = container
            .select(&note_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // Get location
        let position = container
            .select(&location_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        let highlight = Highlight {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            note,
            location: Location {
                chapter: None,
                position,
            },
            created_at: None,
            source: Source::Kindle,
        };

        highlights.push(highlight);
    }

    // Get pagination tokens
    let next_token = document
        .select(&next_page_selector)
        .next()
        .and_then(|e| e.value().attr("value"))
        .map(String::from)
        .filter(|s| !s.is_empty());

    let content_limit_state = document
        .select(&content_limit_selector)
        .next()
        .and_then(|e| e.value().attr("value"))
        .map(String::from)
        .filter(|s| !s.is_empty());

    Ok((highlights, next_token, content_limit_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_amazon_region() {
        let us = LegacyAmazonRegion::from_code("us").unwrap();
        assert_eq!(us.domain, "amazon.com");

        let uk = LegacyAmazonRegion::from_code("UK").unwrap();
        assert_eq!(uk.domain, "amazon.co.uk");

        let invalid = LegacyAmazonRegion::from_code("xyz");
        assert!(invalid.is_err());
    }
}
