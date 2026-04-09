use crate::error::ProviderError;
use crate::ir::{Capability, GIT_READONLY_COMMANDS, ProviderConfig, Sandbox, SandboxTranslator};
use crate::types::{AiEvent, AiProvider, AiRequest, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::json;

/// Configuration for the Copilot CLI provider.
#[derive(Default)]
pub(crate) struct CopilotConfig {
    pub model: Option<String>,
    pub sandbox: Option<Sandbox>,
    pub debug: bool,
}

pub struct CopilotProvider {
    config: CopilotConfig,
}

impl CopilotProvider {
    pub(crate) fn new(config: CopilotConfig) -> Self {
        Self { config }
    }

    pub(crate) fn from_provider_config(config: &ProviderConfig) -> Self {
        Self::new(CopilotConfig {
            model: config.model.clone(),
            sandbox: config.sandbox.clone(),
            debug: config.debug,
        })
    }
}

impl SandboxTranslator for CopilotProvider {
    fn translate_allowed(&self, capabilities: &[Capability]) -> Vec<String> {
        let mut tools = Vec::new();
        for cap in capabilities {
            match cap {
                Capability::GitReadOnly => {
                    for cmd in GIT_READONLY_COMMANDS {
                        tools.push(format!("shell(git:{cmd})"));
                    }
                }
                Capability::ShellCommand { pattern } => {
                    tools.push(format!("shell({pattern})"));
                }
                Capability::Custom(s) => tools.push(s.clone()),
                Capability::ReadFile | Capability::WriteFile | Capability::Network => {}
            }
        }
        tools
    }
}

fn build_system_prompt(base: &str, json_schema: Option<&str>) -> String {
    match json_schema {
        Some(schema) => format!(
            "{base}\n\n\
             You MUST respond with valid JSON matching this schema:\n\
             ```json\n{schema}\n```\n\n\
             Respond ONLY with the JSON object, no markdown fences, no explanation."
        ),
        None => base.to_string(),
    }
}

#[async_trait]
impl AiProvider for CopilotProvider {
    fn name(&self) -> &str {
        "copilot"
    }

    async fn is_available(&self) -> bool {
        Command::new("gh")
            .args(["copilot", "--version"])
            .output()
            .await
            .is_ok_and(|o| o.status.success())
    }

    async fn request(
        &self,
        req: &AiRequest,
        _events: Option<mpsc::UnboundedSender<AiEvent>>,
    ) -> Result<AiResponse> {
        let model = self.config.model.as_deref().unwrap_or("gpt-4.1");
        let system = build_system_prompt(&req.system_prompt, req.json_schema.as_deref());
        let combined_prompt = format!("{system}\n\n{}", req.user_prompt);

        let mut cmd = Command::new("gh");
        cmd.current_dir(&req.working_dir)
            .arg("copilot")
            .arg("-p")
            .arg(&combined_prompt)
            .arg("-s")
            .arg("--model")
            .arg(model);

        if let Some(sandbox) = &self.config.sandbox {
            for tool in self.translate_sandbox(sandbox) {
                cmd.arg("--allow-tool").arg(tool);
            }
        }

        cmd.arg("--no-custom-instructions").arg("--autopilot");

        if self.config.debug {
            eprintln!("[DEBUG] gh copilot (model={model})");
        }

        let output = cmd.output().await.context("failed to run gh copilot")?;
        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr);

        if self.config.debug {
            eprintln!("[DEBUG] exit: {}", output.status);
            eprintln!("[DEBUG] stdout (first 500): {}", &raw[..raw.len().min(500)]);
            if !stderr.is_empty() {
                eprintln!("[DEBUG] stderr: {stderr}");
            }
        }

        if !output.status.success() {
            anyhow::bail!(ProviderError::BackendFailed(format!(
                "gh copilot failed (exit {}): {}",
                output.status,
                stderr.trim()
            )));
        }

        let text = json::extract_json(&raw).unwrap_or(raw);
        Ok(AiResponse { text, usage: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_git_readonly() {
        let provider = CopilotProvider::new(CopilotConfig::default());
        let caps = vec![Capability::GitReadOnly];
        let tools = provider.translate_allowed(&caps);
        assert!(tools.contains(&"shell(git:diff)".to_string()));
        assert!(tools.contains(&"shell(git:log)".to_string()));
        assert_eq!(tools.len(), GIT_READONLY_COMMANDS.len());
    }

    #[test]
    fn translate_custom_passthrough() {
        let provider = CopilotProvider::new(CopilotConfig::default());
        let caps = vec![Capability::Custom("shell(npm:test)".into())];
        let tools = provider.translate_allowed(&caps);
        assert_eq!(tools, vec!["shell(npm:test)"]);
    }
}
