pub mod claude;
pub mod copilot;
pub mod gemini;
pub(crate) mod json;

use crate::error::ProviderError;
use crate::ir::ProviderConfig;
use crate::types::AiProvider;

use claude::ClaudeProvider;
use copilot::CopilotProvider;
use gemini::GeminiProvider;

/// Available local AI backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LocalBackend {
    Claude,
    Copilot,
    Gemini,
}

/// Resolve an available local AI provider, with fallback.
///
/// If a preferred backend is specified but unavailable, falls back to others.
/// If no preference, tries claude -> copilot -> gemini.
pub async fn resolve_local_provider(config: ProviderConfig) -> anyhow::Result<Box<dyn AiProvider>> {
    let claude = ClaudeProvider::from_provider_config(&config);
    let copilot = CopilotProvider::from_provider_config(&config);
    let gemini = GeminiProvider::from_provider_config(&config);

    let try_fallbacks = |backends: Vec<Box<dyn AiProvider>>| async move {
        for backend in backends {
            if backend.is_available().await {
                return Ok(backend);
            }
        }
        anyhow::bail!(ProviderError::NoProviderAvailable)
    };

    match config.backend {
        Some(LocalBackend::Claude) => {
            if claude.is_available().await {
                return Ok(Box::new(claude));
            }
            eprintln!("warning: claude CLI not found, falling back...");
            try_fallbacks(vec![Box::new(copilot), Box::new(gemini)]).await
        }
        Some(LocalBackend::Copilot) => {
            if copilot.is_available().await {
                return Ok(Box::new(copilot));
            }
            eprintln!("warning: gh copilot not available, falling back...");
            try_fallbacks(vec![Box::new(claude), Box::new(gemini)]).await
        }
        Some(LocalBackend::Gemini) => {
            if gemini.is_available().await {
                return Ok(Box::new(gemini));
            }
            eprintln!("warning: gemini CLI not found, falling back...");
            try_fallbacks(vec![Box::new(claude), Box::new(copilot)]).await
        }
        None => try_fallbacks(vec![Box::new(claude), Box::new(copilot), Box::new(gemini)]).await,
    }
}
