use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Spider error: {0}")]
    Spider(String),

    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Gemini API error: {0}")]
    Gemini(String),

    #[error("Environment variable not set: {0}")]
    EnvVar(#[from] std::env::VarError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}


