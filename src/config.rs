use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Output path for the library JSON file
    pub output_path: PathBuf,

    /// Apple Books configuration
    pub apple_books: AppleBooksConfig,

    /// Kindle configuration
    pub kindle: KindleConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_path: default_output_path(),
            apple_books: AppleBooksConfig::default(),
            kindle: KindleConfig::default(),
        }
    }
}

/// Apple Books specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppleBooksConfig {
    /// Whether Apple Books extraction is enabled
    pub enabled: bool,

    /// Override path for the library database
    pub library_db: Option<PathBuf>,

    /// Override path for the annotation database
    pub annotation_db: Option<PathBuf>,
}

impl Default for AppleBooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            library_db: None,
            annotation_db: None,
        }
    }
}

/// Kindle specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KindleConfig {
    /// Whether Kindle extraction is enabled
    pub enabled: bool,

    /// Path to My Clippings.txt file
    pub clippings_path: Option<PathBuf>,

    /// Path to cookies file for Amazon scraping
    pub cookies_path: Option<PathBuf>,

    /// Amazon region code (us, uk, de, fr, etc.)
    pub region: String,
}

impl Default for KindleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            clippings_path: None,
            cookies_path: None,
            region: "us".to_string(),
        }
    }
}

/// Get the default output path
fn default_output_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("readingsync")
        .join("library.json")
}

/// Get the default config file path
pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("readingsync")
        .join("config.toml")
}

impl Config {
    /// Load configuration from a file
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(ConfigError::ReadError)?;

        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }

    /// Load configuration from the default path, falling back to defaults if not found
    pub fn load_default() -> Self {
        let path = default_config_path();
        Self::load(&path).unwrap_or_default()
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::ReadError)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to serialize config: {}", e)))?;

        fs::write(path, content).map_err(ConfigError::ReadError)?;

        Ok(())
    }

    /// Expand tilde in paths
    pub fn expand_paths(&mut self) {
        self.output_path = expand_tilde(&self.output_path);

        if let Some(ref mut path) = self.apple_books.library_db {
            *path = expand_tilde(path);
        }
        if let Some(ref mut path) = self.apple_books.annotation_db {
            *path = expand_tilde(path);
        }
        if let Some(ref mut path) = self.kindle.clippings_path {
            *path = expand_tilde(path);
        }
        if let Some(ref mut path) = self.kindle.cookies_path {
            *path = expand_tilde(path);
        }
    }
}

/// Expand tilde in a path
fn expand_tilde(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path_str[2..]);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.apple_books.enabled);
        assert!(config.kindle.enabled);
        assert_eq!(config.kindle.region, "us");
    }

    #[test]
    fn test_expand_tilde() {
        let path = PathBuf::from("~/test/path");
        let expanded = expand_tilde(&path);

        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home.join("test/path"));
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.kindle.region, config.kindle.region);
    }
}
