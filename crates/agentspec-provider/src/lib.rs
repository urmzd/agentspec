pub mod error;
pub mod ir;
pub mod types;

#[cfg(feature = "local")]
pub mod local;

pub use error::ProviderError;
pub use ir::{Capability, ProviderConfig, Sandbox};
pub use types::{AiEvent, AiProvider, AiRequest, AiResponse, AiUsage};

#[cfg(feature = "local")]
pub use local::{LocalBackend, resolve_local_provider};
