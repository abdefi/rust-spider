use llm::builder::{LLMBackend, LLMBuilder};
use llm::chat::{ChatMessage, StructuredOutputFormat};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Deserialize, Serialize)]
pub struct PatternResponse {
    /// A regex pattern that matches product page URLs.
    /// Empty string if no consistent pattern exists.
    pub pattern: String,
}

/// Builds a minimal JSON schema accepted by the Gemini API for PatternResponse.
fn pattern_response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "A Rust-compatible regex matching product page URLs. Empty string if no pattern found."
            }
        },
        "required": ["pattern"]
    })
}

pub struct GeminiClient {
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gemini-3.1-flash-lite-preview".to_string(),
        }
    }

    /// Sends up to 200 URLs to Gemini and asks it to infer a regex pattern
    /// that identifies product pages.
    /// Returns `None` when Gemini says no consistent pattern exists.
    pub async fn find_product_pattern(
        &self,
        urls: &[String],
    ) -> Result<Option<String>, AppError> {
        let structured_format = StructuredOutputFormat {
            name: "PatternResponse".to_string(),
            description: Some("Product URL pattern extraction result".to_string()),
            schema: Some(pattern_response_schema()),
            strict: None,
        };

        let sample = if urls.len() > 20 { &urls[..20] } else { urls };
        let prompt = format!(
            "You are an expert at recognising e-commerce URL structures.\n\
            \n\
            TASK: return a single Rust-compatible regex that matches EVERY individual \
            product-detail page URL in the list below and rejects everything else.\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            STEP 1 – IDENTIFY THE STRUCTURAL SEPARATOR\n\
            ═══════════════════════════════════════════════════════════════════\n\
            Look at every URL path and determine how the site separates product pages\n\
            from other pages. There are two common patterns:\n\
            \n\
            A) SEGMENT-BASED: products live under a dedicated path segment like\n\
               /product/<slug>, /produkt/<slug>, /lot/<slug>, /item/<slug>, etc.\n\
               Other pages use different segments (/category/, /tag/, /about, …).\n\
               → The regex anchors on that exact segment with slashes.\n\
               → Be careful: /produkt-kategorie/ shares a PREFIX with /produkt/\n\
                 but is NOT a product. Always match the segment as a complete\n\
                 path component between slashes.\n\
            \n\
            B) FLAT / MIXED: all pages (products AND non-products) are top-level\n\
               slugs with no distinguishing path segment.\n\
               → There is NO reliable structural pattern.\n\
               → You MUST return an empty pattern string.\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            STEP 2 – SLUG SUFFIX\n\
            ═══════════════════════════════════════════════════════════════════\n\
            Only if you found a segment in Step 1:\n\
            - If ALL product URLs under that segment end with -<digits only>, \n\
              use  -\\d+$\n\
            - Otherwise use  [\\w%.-]+$  (allow any slug)\n\
            Never require a specific suffix pattern (like -art-\\d+) that only\n\
            appears in SOME product URLs.\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            STEP 3 – STRICT SELF-CHECK  (critical!)\n\
            ═══════════════════════════════════════════════════════════════════\n\
            Mentally test your pattern against EVERY URL in the list.\n\
            a) Every URL that looks like a product detail page MUST match.\n\
               If even ONE product URL does not match → return empty string.\n\
            b) Every URL that is a category, tag, listing, home, utility, \n\
               or pagination page MUST NOT match.\n\
               If even ONE non-product URL matches → try to tighten the pattern.\n\
               If you cannot tighten it without losing products → return empty string.\n\
            \n\
            Prefer returning an empty string over returning a pattern that\n\
            misses products. Recall is more important than precision.\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            OUTPUT\n\
            ═══════════════════════════════════════════════════════════════════\n\
            Return JSON with exactly one field:\n\
            • \"pattern\": the Rust regex, or empty string if no safe pattern exists.\n\
              Pattern rules (only when non-empty):\n\
              - Match the full URL including scheme + domain.\n\
              - Use literal slashes to delimit path segments.\n\
              - End with $.\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            EXAMPLE A – segment-based site (return a pattern)\n\
            ═══════════════════════════════════════════════════════════════════\n\
            URLs:\n\
              https://shop.com/\n\
              https://shop.com/product-category/chairs\n\
              https://shop.com/product-tag/vintage\n\
              https://shop.com/product/oak-chair-12\n\
              https://shop.com/product/pine-desk\n\
            Output:\n\
            {{\n\
              \"pattern\": \"https://shop\\.com/product/[\\\\w%.-]+$\"\n\
            }}\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            EXAMPLE B – flat site (return empty string)\n\
            ═══════════════════════════════════════════════════════════════════\n\
            URLs:\n\
              https://antiques.com/\n\
              https://antiques.com/about-us\n\
              https://antiques.com/baroque-cabinet-art-5012\n\
              https://antiques.com/empire-clock-7039\n\
              https://antiques.com/contact\n\
              https://antiques.com/mahogany-desk\n\
            Output:\n\
            {{\n\
              \"pattern\": \"\"\n\
            }}\n\
            \n\
            ═══════════════════════════════════════════════════════════════════\n\
            ACTUAL URLS TO ANALYSE\n\
            ═══════════════════════════════════════════════════════════════════\n\
            First 20 (for structure context):\n\
            {}\n\
            \n\
            All {} URLs:\n\
            {}",
            sample.join("\n"),
            urls.len(),
            urls.join("\n")
        );

        let messages = vec![ChatMessage::user().content(prompt).build()];

        let llm = LLMBuilder::new()
            .backend(LLMBackend::Google)
            .api_key(self.api_key.clone())
            .model(self.model.clone())
            .temperature(0.0)
            .schema(structured_format)
            .build()
            .map_err(|e| AppError::Gemini(format!("Failed to build Gemini client (structured): {e}")))?;

        let response = match llm.chat(&messages).await {
            Ok(response) => response,
            Err(first_err) => {
                let first_error = first_err.to_string();
                if should_retry_without_schema(&first_error) {
                    eprintln!(
                        "Gemini structured output failed (likely 400/schema issue). Retrying without schema..."
                    );

                    let fallback_llm = LLMBuilder::new()
                        .backend(LLMBackend::Google)
                        .api_key(self.api_key.clone())
                        .model(self.model.clone())
                        .temperature(0.0)
                        .build()
                        .map_err(|e| {
                            AppError::Gemini(format!(
                                "Failed to build Gemini fallback client (no schema): {e}"
                            ))
                        })?;

                    fallback_llm.chat(&messages).await.map_err(|second_err| {
                        AppError::Gemini(format!(
                            "Gemini chat failed with structured output ('{first_error}') and fallback without schema ('{second_err}')."
                        ))
                    })?
                } else {
                    return Err(AppError::Gemini(format!(
                        "Gemini chat error (structured output): {first_error}"
                    )));
                }
            }
        };

        let raw_text = response
            .text()
            .ok_or_else(|| AppError::Gemini("Empty response from Gemini".to_string()))?;

        let parsed = parse_pattern_response(&raw_text)?;


        if parsed.pattern.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(parsed.pattern))
        }
    }
}

fn should_retry_without_schema(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    let is_bad_request = lower.contains("400") || lower.contains("bad request");
    let is_schema_related =
        lower.contains("schema") || lower.contains("structured") || lower.contains("invalid argument");

    is_bad_request || is_schema_related
}

fn parse_pattern_response(raw_text: &str) -> Result<PatternResponse, AppError> {
    if let Ok(parsed) = serde_json::from_str::<PatternResponse>(raw_text) {
        return Ok(parsed);
    }

    let start = raw_text.find('{');
    let end = raw_text.rfind('}');

    if let (Some(s), Some(e)) = (start, end) {
        let json_slice = &raw_text[s..=e];
        let parsed: PatternResponse = serde_json::from_str(json_slice)?;
        return Ok(parsed);
    }

    Err(AppError::Gemini(
        "Could not parse Gemini JSON response".to_string(),
    ))
}
