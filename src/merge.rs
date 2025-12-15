use crate::model::{Book, Highlight};
use std::collections::{HashMap, HashSet};

/// Merge books from multiple sources, deduplicating by book ID and highlight text
pub fn merge_books(book_lists: Vec<Vec<Book>>) -> Vec<Book> {
    let mut books_by_id: HashMap<String, Book> = HashMap::new();

    for books in book_lists {
        for book in books {
            match books_by_id.get_mut(&book.id) {
                Some(existing) => {
                    merge_into_book(existing, book);
                }
                None => {
                    books_by_id.insert(book.id.clone(), book);
                }
            }
        }
    }

    let mut books: Vec<Book> = books_by_id.into_values().collect();

    // Sort books by title
    books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    books
}

/// Merge a book into an existing book entry
fn merge_into_book(existing: &mut Book, other: Book) {
    // Merge sources
    for source in other.sources {
        if !existing.sources.contains(&source) {
            existing.sources.push(source);
        }
    }

    // Merge finished status (true from any source wins)
    if other.finished == Some(true) {
        existing.finished = Some(true);
    } else if existing.finished.is_none() {
        existing.finished = other.finished;
    }

    // Prefer earlier finished_at date
    match (&existing.finished_at, &other.finished_at) {
        (None, Some(_)) => existing.finished_at = other.finished_at,
        (Some(e), Some(o)) if o < e => existing.finished_at = other.finished_at,
        _ => {}
    }

    // Merge highlights, deduplicating by text
    let existing_texts: HashSet<String> = existing
        .highlights
        .iter()
        .map(|h| normalize_text(&h.text))
        .collect();

    for highlight in other.highlights {
        let normalized = normalize_text(&highlight.text);
        if !existing_texts.contains(&normalized) {
            existing.highlights.push(highlight);
        } else {
            // If duplicate, prefer earlier created_at
            merge_duplicate_highlight(&mut existing.highlights, highlight);
        }
    }

    // Sort highlights by created_at
    existing.highlights.sort_by(|a, b| {
        match (&a.created_at, &b.created_at) {
            (Some(a_date), Some(b_date)) => a_date.cmp(b_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

/// Normalize text for comparison (lowercase, collapse whitespace)
fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Merge a duplicate highlight, preferring earlier created_at
fn merge_duplicate_highlight(highlights: &mut Vec<Highlight>, other: Highlight) {
    let normalized_other = normalize_text(&other.text);

    for existing in highlights.iter_mut() {
        if normalize_text(&existing.text) == normalized_other {
            // Prefer earlier created_at
            match (&existing.created_at, &other.created_at) {
                (None, Some(_)) => existing.created_at = other.created_at,
                (Some(e), Some(o)) if o < e => existing.created_at = other.created_at,
                _ => {}
            }

            // Merge note if existing doesn't have one
            if existing.note.is_none() && other.note.is_some() {
                existing.note = other.note;
            }

            // Add source if not present
            // (Note: Highlight has a single source, not a vec, so we can't merge sources here)
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{generate_book_id, Location};
    use chrono::{TimeZone, Utc};

    fn make_book(title: &str, author: Option<&str>, source: Source) -> Book {
        Book {
            id: generate_book_id(title, author),
            title: title.to_string(),
            author: author.map(String::from),
            sources: vec![source],
            highlights: Vec::new(),
            finished: None,
            finished_at: None,
        }
    }

    fn make_highlight(text: &str, source: Source) -> Highlight {
        Highlight {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
            note: None,
            location: Location {
                chapter: None,
                position: None,
            },
            created_at: None,
            source,
        }
    }

    #[test]
    fn test_merge_same_book_different_sources() {
        let mut book1 = make_book("The Great Gatsby", Some("F. Scott Fitzgerald"), Source::AppleBooks);
        book1.highlights.push(make_highlight("Highlight from Apple", Source::AppleBooks));

        let mut book2 = make_book("The Great Gatsby", Some("F. Scott Fitzgerald"), Source::Kindle);
        book2.highlights.push(make_highlight("Highlight from Kindle", Source::Kindle));

        let merged = merge_books(vec![vec![book1], vec![book2]]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sources.len(), 2);
        assert_eq!(merged[0].highlights.len(), 2);
    }

    #[test]
    fn test_merge_duplicate_highlights() {
        let mut book1 = make_book("Test Book", None, Source::AppleBooks);
        book1.highlights.push(make_highlight("Same highlight text", Source::AppleBooks));

        let mut book2 = make_book("Test Book", None, Source::Kindle);
        book2.highlights.push(make_highlight("Same highlight text", Source::Kindle));
        book2.highlights.push(make_highlight("Different highlight", Source::Kindle));

        let merged = merge_books(vec![vec![book1], vec![book2]]);

        assert_eq!(merged.len(), 1);
        // Should have 2 highlights: one deduplicated, one unique
        assert_eq!(merged[0].highlights.len(), 2);
    }

    #[test]
    fn test_finished_status_merge() {
        let mut book1 = make_book("Test Book", None, Source::AppleBooks);
        book1.finished = Some(false);

        let mut book2 = make_book("Test Book", None, Source::Kindle);
        book2.finished = Some(true);

        let merged = merge_books(vec![vec![book1], vec![book2]]);

        assert_eq!(merged[0].finished, Some(true));
    }

    #[test]
    fn test_normalize_text() {
        assert_eq!(
            normalize_text("Hello   World"),
            normalize_text("hello world")
        );
        assert_eq!(
            normalize_text("  Multiple   Spaces  "),
            "multiple spaces"
        );
    }
}
