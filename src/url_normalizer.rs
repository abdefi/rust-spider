use url::Url;

/// Normalizes URLs for stable output and comparison.
///
/// Rules:
/// - remove query and fragment
/// - trim trailing slash (except root)
/// - lowercase scheme and host
/// - keep path as-is
pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();

    let Ok(mut parsed) = Url::parse(trimmed) else {
        return trimmed.trim_end_matches('/').to_string();
    };

    parsed.set_query(None);
    parsed.set_fragment(None);

    let normalized = parsed.to_string();

    // Keep root slash (scheme://host/), trim trailing slash on paths
    if normalized.ends_with("/") && !normalized.ends_with("://") {
        let parts: Vec<&str> = normalized.split("://").collect();
        if parts.len() == 2 {
            let after_scheme = parts[1];
            // Check if this is just domain + single slash
            let has_path = after_scheme.contains("/") && after_scheme.find("/").unwrap() < after_scheme.len() - 1;
            if has_path {
                return normalized.trim_end_matches("/").to_string();
            }
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn normalize_removes_query_and_fragment() {
        let input = "https://www.example.com/product/123?ref=google#reviews";
        let normalized = normalize_url(input);
        assert_eq!(normalized, "https://www.example.com/product/123");
    }

    #[test]
    fn normalize_removes_trailing_slash() {
        let input = "https://www.example.com/products/";
        let normalized = normalize_url(input);
        assert_eq!(normalized, "https://www.example.com/products");
    }

    #[test]
    fn normalize_keeps_root_slash() {
        let input = "https://www.example.com/";
        let normalized = normalize_url(input);
        assert_eq!(normalized, "https://www.example.com/");
    }

    #[test]
    fn normalize_lowercases_scheme_and_host() {
        let input = "HTTPS://WWW.EXAMPLE.COM/Product/123";
        let normalized = normalize_url(input);
        assert!(normalized.starts_with("https://www.example.com/"));
    }

    #[test]
    fn normalize_dedupes_query_variants() {
        let a = normalize_url("https://www.example.com/product/123?ref=google");
        let b = normalize_url("https://www.example.com/product/123?ref=facebook");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_dedupes_fragment_variants() {
        let a = normalize_url("https://www.example.com/product/123#reviews");
        let b = normalize_url("https://www.example.com/product/123#specs");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_differs_for_different_paths() {
        let a = normalize_url("https://www.example.com/product/123");
        let b = normalize_url("https://www.example.com/product/124");
        assert_ne!(a, b);
    }

    #[test]
    fn normalize_handles_malformed_urls() {
        let input = "not a valid url";
        let normalized = normalize_url(input);
        assert_eq!(normalized, "not a valid url");
    }
}
