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
                "description": "A regex pattern matching product page URLs. Empty string if no pattern found."
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
            model: "gemini-2.5-flash".to_string(),
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

        let prompt = format!(
            "You are an expert at analysing e-commerce website URL structures.\n\
            Below is a list of URLs from a single website. Your task is to identify \
            a regex pattern that matches product page URLs.\n\n\
            Rules:\n\
            - Analyse the URL paths for common structural patterns (e.g. /product/, /p/, /item/, /dp/).\n\
            - Return a regex string that would match most product URLs on this site.\n\
            - If you cannot find a consistent pattern, return an EMPTY string for \"pattern\".\n\
            - Return only valid JSON in this exact shape: {{\"pattern\":\"...\"}}\n\n\
            URLs:\n{}",
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
