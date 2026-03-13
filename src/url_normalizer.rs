use url::Url;

/// Normalizes URLs for stable deduplication and comparison.
///
/// Rules:
/// - preserve query string (query params ARE meaningful navigation on PHP sites)
/// - remove fragment only
/// - sort query parameters for stable comparison
/// - trim trailing slash on paths (except bare root)
/// - lowercase scheme and host
pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();

    let Ok(mut parsed) = Url::parse(trimmed) else {
        return trimmed.trim_end_matches('/').to_string();
    };

    // Sort query parameters for stable deduplication
    // e.g. ?id=1&cat=2 and ?cat=2&id=1 are the same page
    if let Some(query) = parsed.query() {
        if !query.is_empty() {
            let mut pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            pairs.sort();
            let sorted_query = pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            parsed.set_query(Some(&sorted_query));
        }
    }

    // Strip fragment only
    parsed.set_fragment(None);

    let normalized = parsed.to_string();

    // Keep root slash (scheme://host/), trim trailing slash on paths
    if normalized.ends_with('/') {
        let without = normalized.trim_end_matches('/');
        // Only strip if there is still a path component after the host
        if without.contains("://") && without.contains('/') {
            let after_scheme = without.splitn(2, "://").nth(1).unwrap_or("");
            if after_scheme.contains('/') {
                return without.to_string();
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
