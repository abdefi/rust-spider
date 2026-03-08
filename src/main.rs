mod error;
mod gemini;
mod heuristics;

use std::collections::HashSet;
use std::env;
use std::sync::Arc;

use regex::Regex;
use spider::website::Website;
use spider::tokio;
use tokio::sync::{oneshot, Mutex};

use crate::error::AppError;
use crate::gemini::GeminiClient;
use crate::heuristics::is_product_page;

const BATCH_SIZE: usize = 100;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();

    let target_url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.antiquitaeten-tuebingen.de".to_string());

    let api_key = env::var("GEMINI_API_KEY")?;

    println!("Crawling: {}", target_url);

    let mut website = Website::new(&target_url);

    if let Ok(chrome_ws_url) = env::var("SPIDER_CHROME_WS_URL") {
        website
            .configuration
            .with_chrome_connection(Some(chrome_ws_url));
    }

    let mut rx = website
        .subscribe(0)
        .ok_or_else(|| AppError::Spider("Failed to subscribe to crawl stream".to_string()))?;

    let streamed_links = Arc::new(Mutex::new(HashSet::<String>::new()));
    let streamed_links_task = Arc::clone(&streamed_links);

    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();

    let collector = tokio::spawn(async move {
        let mut stop_tx = Some(stop_tx);

        while let Ok(page) = rx.recv().await {
            println!("- {}", page.get_url());

            let mut links = streamed_links_task.lock().await;
            links.insert(page.get_url().to_string());

            if links.len() >= BATCH_SIZE {
                if let Some(tx) = stop_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
        }
    });

    let crawl_fut = website.crawl();
    tokio::pin!(crawl_fut);

    let stopped_early = tokio::select! {
        _ = &mut crawl_fut => false,
        result = &mut stop_rx => result.is_ok(),
    };

    // If we stop at batch size, dropping the crawl future cancels crawl progress.
    drop(crawl_fut);
    if stopped_early {
        println!("Batch-Limit erreicht; stoppe Crawl und starte Produkt-URL-Erkennung.");
    }

    if let Err(e) = collector.await {
        eprintln!("Stream collector task failed: {}", e);
    }

    let unique_links: Vec<String> = {
        let links = streamed_links.lock().await;
        links.iter().cloned().collect()
    };

    println!("Streamed {} unique links", unique_links.len());

    if unique_links.is_empty() {
        println!("No links found – exiting.");
        return Ok(());
    }


    let gemini = GeminiClient::new(api_key);

    let mut product_pattern: Option<Regex> = None;

    for chunk in unique_links.chunks(BATCH_SIZE) {
        println!(
            "Asking Gemini about {} URLs …",
            chunk.len()
        );

        match gemini.find_product_pattern(chunk).await {
            Ok(Some(pattern_str)) => {
                println!("Gemini found pattern: {}", pattern_str);
                match Regex::new(&pattern_str) {
                    Ok(re) => {
                        product_pattern = Some(re);
                        break; // Use the first valid pattern we get
                    }
                    Err(e) => {
                        eprintln!("Invalid regex from Gemini ('{}') – {}", pattern_str, e);
                    }
                }
            }
            Ok(None) => {
                println!("Gemini found no pattern in this batch.");
            }
            Err(e) => {
                eprintln!("Gemini error: {}", e);
            }
        }
    }

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let mut confirmed_products: Vec<String> = Vec::new();

    match product_pattern {
        Some(pattern) => {
            // ── Strategy 1: Gemini found a URL pattern ─────────────────────
            // Only keep URLs that match the pattern; skip heuristic HTML check.
            let candidate_urls: Vec<&String> = unique_links
                .iter()
                .filter(|url| pattern.is_match(url))
                .collect();

            println!(
                "{} URLs match the product pattern – confirming via HTTP …",
                candidate_urls.len()
            );

            for url in &candidate_urls {
                match http.get(url.as_str()).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("✓ Product page (pattern): {}", url);
                        confirmed_products.push(url.to_string());
                    }
                    Ok(resp) => {
                        eprintln!("{} returned HTTP {}", url, resp.status());
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch {}: {}", url, e);
                    }
                }
            }
        }
        None => {
            // ── Strategy 2 (fallback): No pattern – use HTML heuristics ────
            println!(
                "No product URL pattern found – falling back to heuristic HTML analysis for all {} URLs …",
                unique_links.len()
            );

            for url in &unique_links {
                match http.get(url.as_str()).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.text().await {
                            Ok(html) => {
                                if is_product_page(&html) {
                                    println!("✓ Product page (heuristic): {}", url);
                                    confirmed_products.push(url.to_string());
                                }
                            }
                            Err(e) => eprintln!("Could not read body of {}: {}", url, e),
                        }
                    }
                    Ok(resp) => {
                        eprintln!("{} returned HTTP {}", url, resp.status());
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch {}: {}", url, e);
                    }
                }
            }
        }
    }

    println!("\n══════════════════════════════════════════");
    println!("Found {} confirmed product pages:", confirmed_products.len());
    for p in &confirmed_products {
        println!("  • {}", p);
    }

    Ok(())
}