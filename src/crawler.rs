use std::collections::HashSet;

use spider::tokio;
use spider::website::Website;
use tokio::sync::mpsc;

use crate::error::AppError;
use crate::url_normalizer::normalize_url;

/// A single crawled page with its normalized URL.
pub struct CrawledPage {
    pub url: String,
}

/// Starts crawling `target_url` in the background and streams deduplicated
/// pages through the returned channel.
///
/// The crawler runs until the website is exhausted or the receiver is dropped.
pub async fn start_crawl(
    target_url: &str,
) -> Result<mpsc::UnboundedReceiver<CrawledPage>, AppError> {
    let (tx, rx) = mpsc::unbounded_channel::<CrawledPage>();

    let mut website = Website::new(target_url);

    // Allow deep crawls and follow query-parameter links (important for PHP shops).
    website
        .with_depth(10)
        .with_budget(Some(spider::hashbrown::HashMap::from([("*", 10_000)])))
        .with_respect_robots_txt(false);

    // Use a large broadcast buffer so fast crawls don't drop messages.
    let mut spider_rx = website
        .subscribe(512)
        .ok_or_else(|| AppError::Spider("Failed to subscribe to crawl stream".to_string()))?;

    // Spider runs in background until exhausted.
    // Must be spawned BEFORE the collector so the broadcast sender exists
    // when the first pages arrive.
    tokio::spawn(async move {
        website.crawl().await;
        website.unsubscribe();
    });

    // Collector: deduplicates and forwards pages over the mpsc channel.
    tokio::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        let mut seen = HashSet::<String>::new();

        loop {
            match spider_rx.recv().await {
                Ok(page) => {
                    let raw_url = page.get_url().to_string();
                    let key = normalize_url(&raw_url);

                    if !seen.insert(key.clone()) {
                        continue;
                    }

                    if tx.send(CrawledPage { url: key }).is_err() {
                        break;
                    }
                }
                // Lagged means we missed some messages – log and keep going
                Err(RecvError::Lagged(n)) => {
                    eprintln!("[crawler] broadcast lagged, dropped {} pages", n);
                }
                // Sender closed → crawl finished
                Err(RecvError::Closed) => break,
            }
        }
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
        };
        assert_eq!(page.url, "https://example.com/product/1");
    }

    #[test]
    fn crawled_page_non_product() {
        let page = CrawledPage {
            url: "https://example.com/about".to_string(),
        };
        assert_eq!(page.url, "https://example.com/about");
    }
}
