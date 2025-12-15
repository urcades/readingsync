use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The complete library export containing all books and highlights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub exported_at: DateTime<Utc>,
    pub books: Vec<Book>,
}

/// A book with its metadata and highlights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    /// SHA256(lowercase(title + author))[:16]
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    /// Which platforms this book was found on
    pub sources: Vec<Source>,
    pub highlights: Vec<Highlight>,
    pub finished: Option<bool>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// A single highlight or annotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    /// From source DB, or generated UUID
    pub id: String,
    pub text: String,
    pub note: Option<String>,
    pub location: Location,
    pub created_at: Option<DateTime<Utc>>,
    /// Which platform this highlight came from
    pub source: Source,
}

/// Location information for a highlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub chapter: Option<String>,
    /// Opaque string, format varies by source
    pub position: Option<String>,
}

/// Source platform for books and highlights
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    AppleBooks,
    Kindle,
}

impl Book {
    /// Create a new book with a generated ID
    pub fn new(title: String, author: Option<String>) -> Self {
        let id = generate_book_id(&title, author.as_deref());
        Self {
            id,
            title,
            author,
            sources: Vec::new(),
            highlights: Vec::new(),
            finished: None,
            finished_at: None,
        }
    }
}

impl Library {
    /// Create a new empty library
    pub fn new() -> Self {
        Self {
            exported_at: Utc::now(),
            books: Vec::new(),
        }
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a book ID from title and author
/// Uses SHA256(lowercase(title + author))[:16]
pub fn generate_book_id(title: &str, author: Option<&str>) -> String {
    use sha2::{Digest, Sha256};

    let normalized_title = title.trim().to_lowercase();
    let normalized_author = author.map(|a| a.trim().to_lowercase()).unwrap_or_default();

    let input = format!("{}{}", normalized_title, normalized_author);
    let hash = Sha256::digest(input.as_bytes());

    // Take first 16 characters of hex representation
    hex::encode(&hash[..8])
}

/// Simple hex encoding for the hash
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_book_id() {
        let id1 = generate_book_id("The Great Gatsby", Some("F. Scott Fitzgerald"));
        let id2 = generate_book_id("the great gatsby", Some("f. scott fitzgerald"));
        let id3 = generate_book_id("  The Great Gatsby  ", Some("  F. Scott Fitzgerald  "));

        // All should produce the same ID due to normalization
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);

        // ID should be 16 characters
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_generate_book_id_no_author() {
        let id1 = generate_book_id("Some Book", None);
        let id2 = generate_book_id("some book", None);

        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }
}
