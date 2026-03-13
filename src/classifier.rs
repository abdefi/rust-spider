use std::collections::HashSet;

use regex::Regex;

use crate::error::AppError;
use crate::gemini::GeminiClient;
use crate::url_normalizer::normalize_url;

/// Asks Gemini once to derive a product-URL regex from the given URLs.
/// Returns `None` if Gemini finds no consistent pattern.
pub async fn find_gemini_pattern(
    gemini: &GeminiClient,
    all_urls: &[String],
) -> Result<Option<Regex>, AppError> {
    println!("[Gemini Classification] Analyzing {} URLs …", all_urls.len());

    match gemini.find_product_pattern(all_urls).await {
        Ok(Some(pattern_str)) => {
            println!("✓ Gemini returned pattern: {}", pattern_str);
            match Regex::new(&pattern_str) {
                Ok(re) => {
                    println!("✓ Pattern compiled successfully.");
                    Ok(Some(re))
                }
                Err(e) => {
                    eprintln!("⚠ Invalid regex from Gemini ('{}') – {}", pattern_str, e);
                    Ok(None)
                }
            }
        }
        Ok(None) => {
            println!("Gemini found no pattern in these URLs.");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Classifies a single URL against a pattern.
pub fn classify_url(pattern: &Option<Regex>, url: &str) -> bool {
    match pattern {
        Some(re) => re.is_match(url),
        None => false,
    }
}

/// Applies the pattern to all URLs and returns confirmed product pages.
/// Returns `Err(NoProducts)` when no URLs match.
///
/// Designed to stay modular: plug in a different `Strategy` enum variant here
/// in the future without changing the call-sites in `main`.
pub fn apply_strategy(
    pattern: &Option<Regex>,
    all_urls: &[String],
) -> Result<Vec<String>, AppError> {
    match pattern {
        Some(re) => {
            println!("Applying Gemini pattern to {} URLs …", all_urls.len());
            let matches: Vec<String> = all_urls
                .iter()
                .filter(|url| re.is_match(url))
                .cloned()
                .collect();
            println!("✓ {} URLs matched the pattern.", matches.len());

            if matches.is_empty() {
                return Err(AppError::NoProducts(
                    "Gemini pattern matched 0 URLs".to_string(),
                ));
            }

            Ok(dedupe_urls(matches))
        }
        None => Err(AppError::NoProducts(
            "No pattern available – classification skipped".to_string(),
        )),
    }
}

/// Deduplicates URLs by canonical key and returns normalized, unique list.
fn dedupe_urls(urls: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut unique = Vec::new();

    for raw in urls {
        let key = normalize_url(&raw);
        if seen.insert(key) {
            unique.push(normalize_url(&raw));
        }
    }

    unique
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_url_with_pattern() {
        let pattern = Regex::new(r"/product/\d+").ok();
        assert!(classify_url(&pattern, "https://example.com/product/123"));
        assert!(!classify_url(&pattern, "https://example.com/about"));
    }

    #[test]
    fn classify_url_without_pattern() {
        let pattern: Option<Regex> = None;
        assert!(!classify_url(&pattern, "https://example.com/product/123"));
    }

    #[test]
    fn dedupe_urls_removes_duplicates() {
        let urls = vec![
            "https://example.com/product/1".to_string(),
            "https://example.com/product/1?ref=google".to_string(),
            "https://example.com/product/2".to_string(),
        ];
        let deduped = dedupe_urls(urls);
        assert_eq!(deduped.len(), 2);
        assert!(deduped.contains(&"https://example.com/product/1".to_string()));
        assert!(deduped.contains(&"https://example.com/product/2".to_string()));
    }

    #[test]
    fn apply_strategy_no_pattern_returns_error() {
        let pattern: Option<Regex> = None;
        let all_urls = vec!["https://example.com/product/1".to_string()];
        let result = apply_strategy(&pattern, &all_urls);
        assert!(result.is_err());
    }

    #[test]
    fn apply_strategy_with_pattern_throws_on_empty() {
        let pattern = Regex::new(r"/nomatch").ok();
        let all_urls = vec!["https://example.com/product/1".to_string()];
        let result = apply_strategy(&pattern, &all_urls);
        assert!(result.is_err());
        match result {
            Err(AppError::NoProducts(msg)) => assert!(msg.contains("pattern matched 0")),
            _ => panic!("Expected NoProducts error"),
        }
    }

    #[test]
    fn apply_strategy_with_pattern_succeeds() {
        let pattern = Regex::new(r"/product/\d+").ok();
        let all_urls = vec![
            "https://example.com/product/1".to_string(),
            "https://example.com/product/2".to_string(),
        ];
        let result = apply_strategy(&pattern, &all_urls);
        assert!(result.is_ok());
        let products = result.unwrap();
        assert_eq!(products.len(), 2);
    }
}
