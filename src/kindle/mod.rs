pub mod clippings;
pub mod scraper;

pub use clippings::parse_clippings;
pub use scraper::{AmazonRegion, scrape_highlights};
