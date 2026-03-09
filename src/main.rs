mod classifier;
mod config;
mod crawler;
mod error;
mod gemini;
mod heuristics;
mod url_normalizer;

use spider::tokio;

use crate::classifier::{apply_strategy, classify_url, find_gemini_pattern};
use crate::config::AppConfig;
use crate::crawler::start_crawl;
use crate::error::AppError;
use crate::gemini::GeminiClient;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();

    let cfg = AppConfig::from_env()?;

    println!("Starting crawler: {}", cfg.target_url);
    println!("Classification threshold: {} URLs\n", cfg.classify_threshold);

    // ── Phase 1: Start crawling in background ──────────────────────────────
    let mut rx = start_crawl(&cfg.target_url, cfg.chrome_ws_url).await?;

    // ── Phase 2: Collect URLs until threshold, then classify ───────────────
    let mut all_urls = Vec::new();
    let mut heuristic_products = Vec::new();
    let mut total_crawled = 0;
    let mut classification_done = false;
    let mut pattern = None;

    let gemini = GeminiClient::new(cfg.api_key);

    while let Some(page) = rx.recv().await {
        total_crawled += 1;
        all_urls.push(page.url.clone());

        // ── Classify once when threshold reached ─────────────────────────
        if !classification_done && all_urls.len() >= cfg.classify_threshold {
            println!("\nThreshold reached – classifying {} URLs with Gemini …", all_urls.len());
            pattern = find_gemini_pattern(&gemini, &all_urls).await?;
            classification_done = true;

            if pattern.is_some() {
                heuristic_products.clear();
                for (i, url) in all_urls.iter().enumerate() {
                    if classify_url(&pattern, url) {
                        println!("  ✓ [{}] {}", i + 1, url);
                    } else {
                        println!("  ✗ [{}] {}", i + 1, url);
                    }
                }
                let already_matched = all_urls.iter().filter(|u| classify_url(&pattern, u)).count();
                println!("   → {} of the first {} URLs are product pages\n", already_matched, all_urls.len());
            } else {
                for (i, url) in all_urls.iter().enumerate() {
                    if heuristic_products.contains(url) {
                        println!("  ✓ [{}] (heuristic) {}", i + 1, url);
                    } else {
                        println!("  ✗ [{}] {}", i + 1, url);
                    }
                }
                println!("   → No Gemini pattern found. Falling back to heuristics ({} pages so far)\n", heuristic_products.len());
            }
        }

        // ── Track / live-log matches ──────────────────────────────────────
        if classification_done {
            if pattern.is_some() {
                if classify_url(&pattern, &page.url) {
                    println!("  ✓ [{}] {}", total_crawled, page.url);
                } else {
                    println!("  ✗ [{}] {}", total_crawled, page.url);
                }
            } else if page.is_heuristic_product {
                heuristic_products.push(page.url.clone());
                println!("  ✓ [{}] (heuristic) {}", total_crawled, page.url);
            } else {
                println!("  ✗ [{}] {}", total_crawled, page.url);
            }
        } else if page.is_heuristic_product {
            heuristic_products.push(page.url.clone());
        }

        // ── Progress log every 100 URLs (printed after processing) ───────
        if total_crawled % 100 == 0 {
            let products_so_far = if classification_done {
                if pattern.is_some() {
                    all_urls.iter().filter(|u| classify_url(&pattern, u)).count()
                } else {
                    heuristic_products.len()
                }
            } else {
                heuristic_products.len()
            };
            println!(
                "[{} URLs crawled] {} product pages identified so far …",
                total_crawled, products_so_far
            );
        }
    }

    println!("\nCrawl complete. Total URLs: {}", total_crawled);

    // ── Phase 3: Apply final classification ────────────────────────────────
    println!("\nFinal classification…");
    let confirmed_products = apply_strategy(&pattern, &all_urls, &heuristic_products)?;

    println!("\n══════════════════════════════════════════");
    println!("Found {} confirmed product pages:", confirmed_products.len());
    for p in &confirmed_products {
        println!("  • {}", p);
    }

    Ok(())
}