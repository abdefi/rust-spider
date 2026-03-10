use std::collections::HashSet;

use spider::tokio;
use spider::website::Website;
use tokio::sync::mpsc;

use crate::error::AppError;
use crate::heuristics::is_product_page;
use crate::url_normalizer::normalize_url;

/// A single crawled page with its normalized URL and a lightweight heuristic flag.
/// The flag is computed once in the crawler (where HTML is already in memory)
/// so that the HTML itself never needs to be stored.
pub struct CrawledPage {
    pub url: String,
    /// Pre-computed heuristic result – only consulted when Gemini fails.
    pub is_heuristic_product: bool,
}

/// Starts crawling `target_url` in the background and streams deduplicated
/// pages through the returned channel.
///
/// The crawler runs until the website is exhausted or the receiver is dropped.
pub async fn start_crawl(
    target_url: &str,
    chrome_ws_url: Option<String>,
) -> Result<mpsc::UnboundedReceiver<CrawledPage>, AppError> {
    let (tx, rx) = mpsc::unbounded_channel::<CrawledPage>();

    let mut website = Website::new(target_url);

    // Crawler configuration
    website.configuration.respect_robots_txt = true;
    website.configuration.subdomains = false;
    website.configuration.tld = false;
    website.configuration.with_depth(250);
    website.configuration.with_user_agent(Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".into()));
    website.configuration.delay = 50; // polite 50 ms delay
    website.configuration.with_request_timeout(Some(std::time::Duration::from_secs(30)));

    if let Some(ws_url) = chrome_ws_url {
        website.configuration.with_chrome_connection(Some(ws_url));
    }

    let mut spider_rx = website
        .subscribe(0)
        .ok_or_else(|| AppError::Spider("Failed to subscribe to crawl stream".to_string()))?;

    // Collector: deduplicates and forwards pages over the channel
    tokio::spawn(async move {
        let mut seen = HashSet::<String>::new();

        while let Ok(page) = spider_rx.recv().await {
            let raw_url = page.get_url().to_string();
            let key = normalize_url(&raw_url);

            if !seen.insert(key) {
                continue;
            }

            let url = normalize_url(&raw_url);
            let html = page.get_html();
            let is_heuristic_product = !html.is_empty() && is_product_page(&html);

            // If receiver is dropped (main loop done), stop collecting
            if tx.send(CrawledPage { url, is_heuristic_product }).is_err() {
                break;
            }
        }
    });

    // Spider runs in background until exhausted
    tokio::spawn(async move {
        website.crawl().await;
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crawled_page_creation() {
        let page = CrawledPage {
            url: "https://example.com/product/1".to_string(),
            is_heuristic_product: true,
        };
        assert_eq!(page.url, "https://example.com/product/1");
        assert!(page.is_heuristic_product);
    }

    #[test]
    fn crawled_page_non_product() {
        let page = CrawledPage {
            url: "https://example.com/about".to_string(),
            is_heuristic_product: false,
        };
        assert_eq!(page.url, "https://example.com/about");
        assert!(!page.is_heuristic_product);
    }
}

