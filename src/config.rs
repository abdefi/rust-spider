use std::env;

use crate::error::AppError;

/// All runtime configuration read once at startup.
pub struct AppConfig {
    /// The website URL to crawl (always starts with https://).
    pub target_url: String,
    /// Gemini API key.
    pub api_key: String,
    /// Optional Chrome DevTools WebSocket URL for JavaScript-rendered pages.
    pub chrome_ws_url: Option<String>,
    /// Number of URLs to collect before triggering the one-time Gemini classification.
    pub classify_threshold: usize,
}

impl AppConfig {
    /// Reads configuration from CLI arguments and environment variables.
    ///
    /// # Errors
    /// Returns [`AppError::EnvVar`] when `GEMINI_API_KEY` is not set.
    pub fn from_env() -> Result<Self, AppError> {
        let raw_url = env::args()
            .nth(1)
            .unwrap_or_else(|| "https://www.christies.com/en".to_string());

        let target_url = ensure_scheme(&raw_url);
        let api_key = env::var("GEMINI_API_KEY")?;
        let chrome_ws_url = env::var("SPIDER_CHROME_WS_URL").ok();

        Ok(Self {
            target_url,
            api_key,
            chrome_ws_url,
            classify_threshold: 200,
        })
    }
}

/// Ensures the URL starts with `https://`. Prepends it if missing.
fn ensure_scheme(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(url: &str) -> AppConfig {
        AppConfig {
            target_url: ensure_scheme(url),
            api_key: "test-key".to_string(),
            chrome_ws_url: None,
            classify_threshold: 200,
        }
    }

    #[test]
    fn ensure_scheme_prepends_https_when_missing() {
        assert_eq!(ensure_scheme("example.com"), "https://example.com");
        assert_eq!(ensure_scheme("www.example.com/shop"), "https://www.example.com/shop");
    }

    #[test]
    fn ensure_scheme_keeps_existing_https() {
        assert_eq!(ensure_scheme("https://example.com"), "https://example.com");
    }

    #[test]
    fn ensure_scheme_keeps_existing_http() {
        assert_eq!(ensure_scheme("http://example.com"), "http://example.com");
    }

    #[test]
    fn ensure_scheme_trims_whitespace() {
        assert_eq!(ensure_scheme("  example.com  "), "https://example.com");
    }

    #[test]
    fn config_has_correct_threshold() {
        let cfg = make_config("example.com");
        assert_eq!(cfg.classify_threshold, 200);
    }

    #[test]
    fn config_normalizes_url() {
        let cfg = make_config("nostalgie-palast.de");
        assert_eq!(cfg.target_url, "https://nostalgie-palast.de");
    }
}

