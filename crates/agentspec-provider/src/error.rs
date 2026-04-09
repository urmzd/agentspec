use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("AI backend failed: {0}")]
    BackendFailed(String),

    #[error("no AI provider available (install `claude` or `gemini` CLI)")]
    NoProviderAvailable,

    #[error("failed to parse AI response: {0}")]
    ParseResponse(String),
}
