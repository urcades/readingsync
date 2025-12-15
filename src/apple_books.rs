use crate::error::AppleBooksError;
use crate::model::{generate_book_id, Book, Highlight, Location, Source};
use chrono::{TimeZone, Utc};
use glob::glob;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// CoreData epoch offset (2001-01-01 00:00:00 UTC)
const CORE_DATA_EPOCH_OFFSET: i64 = 978307200;

/// Default paths for Apple Books databases
const LIBRARY_DB_PATTERN: &str =
    "~/Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary/BKLibrary*.sqlite";
const ANNOTATION_DB_PATTERN: &str =
    "~/Library/Containers/com.apple.iBooksX/Data/Documents/AEAnnotation/AEAnnotation*.sqlite";

/// Find a database file matching the glob pattern
fn find_database(pattern: &str) -> Option<PathBuf> {
    let expanded = shellexpand::tilde(pattern);
    glob(&expanded)
        .ok()?
        .filter_map(|r| r.ok())
        .filter(|p| !p.to_string_lossy().contains("-wal") && !p.to_string_lossy().contains("-shm"))
        .next()
}

/// Copy database to a temp location to avoid lock issues
fn copy_to_temp(source: &PathBuf) -> Result<PathBuf, AppleBooksError> {
    let temp_dir = std::env::temp_dir();
    let file_name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let temp_path = temp_dir.join(format!("bookexport_{}", file_name));

    fs::copy(source, &temp_path).map_err(AppleBooksError::TempCopyFailed)?;

    Ok(temp_path)
}


/// Full extraction with proper asset_id handling
pub fn extract_full(
    library_db_path: Option<PathBuf>,
    annotation_db_path: Option<PathBuf>,
) -> Result<Vec<Book>, AppleBooksError> {
    // Find or use provided database paths
    let library_db = library_db_path
        .or_else(|| find_database(LIBRARY_DB_PATTERN))
        .ok_or(AppleBooksError::NoDatabasesFound)?;

    let annotation_db = annotation_db_path
        .or_else(|| find_database(ANNOTATION_DB_PATTERN))
        .ok_or(AppleBooksError::NoDatabasesFound)?;

    // Copy databases to temp location
    let temp_library_db = copy_to_temp(&library_db)?;
    let temp_annotation_db = copy_to_temp(&annotation_db)?;

    // Extract books with asset_id
    let conn = Connection::open(&temp_library_db)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            ZASSETID,
            ZTITLE,
            ZAUTHOR,
            ZISFINISHED,
            ZDATEFINISHED
        FROM ZBKLIBRARYASSET
        WHERE ZTITLE IS NOT NULL
        "#,
    )?;

    let mut books_by_asset: HashMap<String, Book> = HashMap::new();

    let rows = stmt.query_map([], |row| {
        let asset_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let author: Option<String> = row.get(2)?;
        let is_finished: Option<i64> = row.get(3)?;
        let finished_timestamp: Option<f64> = row.get(4)?;

        let finished_at = finished_timestamp.and_then(|ts| {
            let unix_ts = ts as i64 + CORE_DATA_EPOCH_OFFSET;
            Utc.timestamp_opt(unix_ts, 0).single()
        });

        Ok((asset_id, title, author, is_finished, finished_at))
    })?;

    for row_result in rows {
        let (asset_id, title, author, is_finished, finished_at) = row_result?;
        let id = generate_book_id(&title, author.as_deref());

        let book = Book {
            id,
            title,
            author,
            sources: vec![Source::AppleBooks],
            highlights: Vec::new(),
            finished: Some(is_finished.unwrap_or(0) == 1),
            finished_at,
        };

        books_by_asset.insert(asset_id, book);
    }

    drop(stmt);
    drop(conn);

    // Extract annotations
    let conn = Connection::open(&temp_annotation_db)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            ZANNOTATIONUUID,
            ZANNOTATIONASSETID,
            ZANNOTATIONSELECTEDTEXT,
            ZANNOTATIONNOTE,
            ZFUTUREPROOFING5,
            ZANNOTATIONLOCATION,
            ZANNOTATIONCREATIONDATE
        FROM ZAEANNOTATION
        WHERE ZANNOTATIONDELETED = 0
          AND ZANNOTATIONSELECTEDTEXT IS NOT NULL
          AND ZANNOTATIONSELECTEDTEXT != ''
        ORDER BY ZANNOTATIONASSETID, ZPLLOCATIONRANGESTART
        "#,
    )?;

    let annotation_rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let asset_id: String = row.get(1)?;
        let text: String = row.get(2)?;
        let note: Option<String> = row.get(3)?;
        let chapter: Option<String> = row.get(4)?;
        let position: Option<String> = row.get(5)?;
        let created_timestamp: Option<f64> = row.get(6)?;

        let created_at = created_timestamp.and_then(|ts| {
            let unix_ts = ts as i64 + CORE_DATA_EPOCH_OFFSET;
            Utc.timestamp_opt(unix_ts, 0).single()
        });

        Ok((id, asset_id, text, note, chapter, position, created_at))
    })?;

    for row_result in annotation_rows {
        let (id, asset_id, text, note, chapter, position, created_at) = row_result?;

        if let Some(book) = books_by_asset.get_mut(&asset_id) {
            let highlight = Highlight {
                id,
                text,
                note,
                location: Location { chapter, position },
                created_at,
                source: Source::AppleBooks,
            };
            book.highlights.push(highlight);
        }
    }

    // Clean up temp files
    let _ = fs::remove_file(&temp_library_db);
    let _ = fs::remove_file(&temp_annotation_db);

    Ok(books_by_asset.into_values().collect())
}

// Use shellexpand for tilde expansion
mod shellexpand {
    pub fn tilde(path: &str) -> String {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return format!("{}{}", home.display(), &path[1..]);
            }
        }
        path.to_string()
    }
}
