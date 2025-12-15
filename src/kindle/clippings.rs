use crate::error::KindleError;
use crate::model::{generate_book_id, Book, Highlight, Location, Source};
use chrono::{DateTime, TimeZone, Utc};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parse Kindle's My Clippings.txt file
///
/// Format:
/// ```text
/// Book Title (Author Name)
/// - Your Highlight on Location 123-145 | Added on Monday, January 1, 2024
///
/// The actual highlighted text goes here...
/// ==========
/// ```
pub fn parse_clippings(path: &Path) -> Result<Vec<Book>, KindleError> {
    if !path.exists() {
        return Err(KindleError::ClippingsFileNotFound(path.to_path_buf()));
    }

    let content = fs::read_to_string(path).map_err(KindleError::ClippingsReadError)?;

    parse_clippings_content(&content)
}

/// Parse the content of a clippings file
pub fn parse_clippings_content(content: &str) -> Result<Vec<Book>, KindleError> {
    let entries = content.split("==========").filter(|s| !s.trim().is_empty());

    let mut books_map: HashMap<String, Book> = HashMap::new();

    for entry in entries {
        if let Some(clipping) = parse_clipping_entry(entry) {
            let book_id = generate_book_id(&clipping.book_title, clipping.author.as_deref());

            let book = books_map.entry(book_id.clone()).or_insert_with(|| Book {
                id: book_id,
                title: clipping.book_title.clone(),
                author: clipping.author.clone(),
                sources: vec![Source::Kindle],
                highlights: Vec::new(),
                finished: None,
                finished_at: None,
            });

            // Only add highlights, skip bookmarks
            if clipping.clipping_type == ClippingType::Highlight
                || clipping.clipping_type == ClippingType::Note
            {
                let highlight = Highlight {
                    id: uuid::Uuid::new_v4().to_string(),
                    text: clipping.content,
                    note: if clipping.clipping_type == ClippingType::Note {
                        None // Notes have the text as the main content
                    } else {
                        None
                    },
                    location: Location {
                        chapter: None,
                        position: clipping.location,
                    },
                    created_at: clipping.added_on,
                    source: Source::Kindle,
                };
                book.highlights.push(highlight);
            }
        }
    }

    Ok(books_map.into_values().collect())
}

#[derive(Debug, PartialEq)]
enum ClippingType {
    Highlight,
    Note,
    Bookmark,
}

#[derive(Debug)]
struct Clipping {
    book_title: String,
    author: Option<String>,
    clipping_type: ClippingType,
    location: Option<String>,
    added_on: Option<DateTime<Utc>>,
    content: String,
}

/// Parse a single clipping entry
fn parse_clipping_entry(entry: &str) -> Option<Clipping> {
    let lines: Vec<&str> = entry.trim().lines().collect();

    if lines.len() < 2 {
        return None;
    }

    // First line: Book Title (Author Name)
    let (book_title, author) = parse_title_author(lines[0]);

    // Second line: - Your Highlight on Location 123-145 | Added on Monday, January 1, 2024
    let (clipping_type, location, added_on) = parse_metadata(lines[1])?;

    // Rest is the content (skip empty lines at the start)
    let content_lines: Vec<&str> = lines[2..].iter().skip_while(|l| l.is_empty()).copied().collect();
    let content = content_lines.join("\n").trim().to_string();

    if content.is_empty() && clipping_type != ClippingType::Bookmark {
        return None;
    }

    Some(Clipping {
        book_title,
        author,
        clipping_type,
        location,
        added_on,
        content,
    })
}

/// Parse the title and author from the first line
fn parse_title_author(line: &str) -> (String, Option<String>) {
    let line = line.trim();

    // Match pattern: "Title (Author)"
    let re = Regex::new(r"^(.+?)\s*\(([^)]+)\)\s*$").unwrap();

    if let Some(caps) = re.captures(line) {
        let title = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap();
        let author = caps.get(2).map(|m| m.as_str().trim().to_string());
        (title, author)
    } else {
        // No author in parentheses
        (line.to_string(), None)
    }
}

/// Parse the metadata line (type, location, date)
fn parse_metadata(line: &str) -> Option<(ClippingType, Option<String>, Option<DateTime<Utc>>)> {
    let line = line.trim();

    // Determine clipping type
    let clipping_type = if line.contains("Highlight") {
        ClippingType::Highlight
    } else if line.contains("Note") {
        ClippingType::Note
    } else if line.contains("Bookmark") {
        ClippingType::Bookmark
    } else {
        return None;
    };

    // Extract location
    let location = extract_location(line);

    // Extract date
    let added_on = extract_date(line);

    Some((clipping_type, location, added_on))
}

/// Extract location from metadata line
fn extract_location(line: &str) -> Option<String> {
    // Match patterns like "Location 123-145" or "Location 123" or "page 45"
    let re = Regex::new(r"(?i)(?:Location|Loc\.|page)\s*(\d+(?:-\d+)?)").unwrap();

    re.captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract date from metadata line
fn extract_date(line: &str) -> Option<DateTime<Utc>> {
    // Match pattern: "Added on Day, Month DD, YYYY HH:MM:SS AM/PM"
    // or "Added on Day, DD Month YYYY HH:MM:SS" (international format)

    // Try common formats
    let date_patterns = [
        // "Added on Monday, January 1, 2024 12:00:00 PM"
        r"Added on \w+,\s*(\w+)\s+(\d+),\s*(\d{4})",
        // "Added on Monday, 1 January 2024"
        r"Added on \w+,\s*(\d+)\s+(\w+)\s+(\d{4})",
    ];

    for pattern in date_patterns {
        let re = Regex::new(pattern).ok()?;
        if let Some(caps) = re.captures(line) {
            // Try to parse the date
            // This is simplified - full implementation would handle all cases
            if let Some(year) = caps.get(3) {
                if let Ok(year_num) = year.as_str().parse::<i32>() {
                    // Return a rough date (just the year for now)
                    if let Some(dt) = Utc.with_ymd_and_hms(year_num, 1, 1, 0, 0, 0).single() {
                        return Some(dt);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_author() {
        let (title, author) = parse_title_author("The Great Gatsby (F. Scott Fitzgerald)");
        assert_eq!(title, "The Great Gatsby");
        assert_eq!(author, Some("F. Scott Fitzgerald".to_string()));

        let (title, author) = parse_title_author("Some Book Without Author");
        assert_eq!(title, "Some Book Without Author");
        assert_eq!(author, None);
    }

    #[test]
    fn test_parse_clippings_content() {
        let content = r#"
The Great Gatsby (F. Scott Fitzgerald)
- Your Highlight on Location 123-145 | Added on Monday, January 1, 2024

In my younger and more vulnerable years my father gave me some advice.
==========
The Great Gatsby (F. Scott Fitzgerald)
- Your Highlight on Location 200-210 | Added on Monday, January 1, 2024

So we beat on, boats against the current.
==========
"#;

        let books = parse_clippings_content(content).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "The Great Gatsby");
        assert_eq!(books[0].highlights.len(), 2);
    }

    #[test]
    fn test_extract_location() {
        assert_eq!(
            extract_location("- Your Highlight on Location 123-145"),
            Some("123-145".to_string())
        );
        assert_eq!(
            extract_location("- Your Highlight on Location 123"),
            Some("123".to_string())
        );
        assert_eq!(
            extract_location("- Your Highlight on page 45"),
            Some("45".to_string())
        );
    }
}
