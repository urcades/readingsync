use std::path::PathBuf;
use thiserror::Error;

/// Main error type for the bookexport application
#[derive(Error, Debug)]
pub enum Error {
    #[error("Apple Books error: {0}")]
    AppleBooks(#[from] AppleBooksError),

    #[error("Kindle error: {0}")]
    Kindle(#[from] KindleError),

    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Errors specific to Apple Books extraction
#[derive(Error, Debug)]
pub enum AppleBooksError {
    #[error("Apple Books library database not found at {0}")]
    LibraryDbNotFound(PathBuf),

    #[error("Apple Books annotation database not found at {0}")]
    AnnotationDbNotFound(PathBuf),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Failed to copy database to temp location: {0}")]
    TempCopyFailed(std::io::Error),

    #[error("No Apple Books databases found")]
    NoDatabasesFound,
}

/// Errors specific to Kindle extraction
#[derive(Error, Debug)]
pub enum KindleError {
    #[error("Clippings file not found: {0}")]
    ClippingsFileNotFound(PathBuf),

    #[error("Failed to read clippings file: {0}")]
    ClippingsReadError(std::io::Error),

    #[error("Failed to parse clipping entry: {0}")]
    ClippingsParseError(String),

    #[error("Cookie file not found: {0}")]
    CookieFileNotFound(PathBuf),

    #[error("Failed to load cookies: {0}")]
    CookieLoadError(String),

    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Failed to parse Amazon page: {0}")]
    ParseError(String),

    #[error("Not authenticated with Amazon. Please provide valid cookies.")]
    NotAuthenticated,

    #[error("Invalid Amazon region: {0}")]
    InvalidRegion(String),
}

/// Errors specific to configuration
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to read config file: {0}")]
    ReadError(std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid config value: {0}")]
    InvalidValue(String),
}

pub type Result<T> = std::result::Result<T, Error>;
