pub mod browser;
pub mod clippings;
pub mod scraper;

pub use browser::{AmazonRegion, BrowserConfig, KindleBrowserScraper};
pub use clippings::parse_clippings;
pub use scraper::scrape_highlights;
