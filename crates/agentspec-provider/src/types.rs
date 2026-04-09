use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A request to an AI provider.
#[derive(Debug, Clone)]
pub struct AiRequest {
    /// System prompt that sets the AI's behavior.
    pub system_prompt: String,
    /// User prompt with the actual task.
    pub user_prompt: String,
    /// Optional JSON schema for structured output.
    pub json_schema: Option<String>,
    /// Working directory for tool execution.
    pub working_dir: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
}

/// A response from an AI provider.
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub text: String,
    pub usage: Option<AiUsage>,
}

/// Real-time events emitted during an AI request.
#[derive(Debug, Clone)]
pub enum AiEvent {
    /// A tool was called by the AI.
    ToolCall { tool: String, input: String },
}

/// Trait for AI backends that can process requests.
///
/// Implementors provide access to an AI model, either through a local CLI tool
/// (Tier 1) or a direct HTTP API client (Tier 2, future).
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Human-readable name of this provider (e.g., "claude", "gemini").
    fn name(&self) -> &str;

    /// Check whether this provider is available (e.g., CLI tool is installed).
    async fn is_available(&self) -> bool;

    /// Send a request to the AI provider.
    ///
    /// If `events` is provided, real-time events (like tool calls) are sent
    /// through the channel during processing.
    async fn request(
        &self,
        req: &AiRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AiEvent>>,
    ) -> anyhow::Result<AiResponse>;
}
