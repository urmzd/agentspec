pub mod adapter;
pub mod adapters;
pub mod discover;
pub mod find;
pub mod ir;
pub mod render;
pub mod sync;

use crate::error::{AppError, Result};

/// Look up a session adapter by user-provided name (with aliases).
pub fn get_adapter(name: &str) -> Result<Box<dyn adapter::SessionAdapter>> {
    adapters::adapter_for_tool(name).ok_or_else(|| {
        AppError::Other(format!(
            "Unknown source: {name}. Supported: claude, codex, copilot, gemini"
        ))
    })
}
