use crate::error::ProviderError;
use crate::ir::{Capability, GIT_READONLY_COMMANDS, ProviderConfig, Sandbox, SandboxTranslator};
use crate::types::{AiEvent, AiProvider, AiRequest, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::io::Write as _;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::json;

/// Configuration for the Gemini CLI provider.
pub(crate) struct GeminiConfig {
    pub model: Option<String>,
    pub sandbox: Option<Sandbox>,
    pub debug: bool,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            model: None,
            sandbox: None,
            debug: false,
        }
    }
}

pub struct GeminiProvider {
    config: GeminiConfig,
}

impl GeminiProvider {
    pub(crate) fn new(config: GeminiConfig) -> Self {
        Self { config }
    }

    pub(crate) fn from_provider_config(config: &ProviderConfig) -> Self {
        Self::new(GeminiConfig {
            model: config.model.clone(),
            sandbox: config.sandbox.clone(),
            debug: config.debug,
        })
    }

    fn apply_sandbox(&self, cmd: &mut Command) -> Result<Option<tempfile::NamedTempFile>> {
        if let Some(sandbox) = &self.config.sandbox {
            let policy = self.translate_sandbox_to_toml(sandbox);
            if policy.is_empty() {
                return Ok(None);
            }
            let mut file =
                tempfile::NamedTempFile::new().context("failed to create policy temp file")?;
            file.write_all(policy.as_bytes())
                .context("failed to write policy file")?;
            cmd.arg("--sandbox").arg("--policy").arg(file.path());
            Ok(Some(file))
        } else {
            Ok(None)
        }
    }

    async fn request_streaming(
        &self,
        req: &AiRequest,
        events: mpsc::UnboundedSender<AiEvent>,
    ) -> Result<AiResponse> {
        let prompt = build_prompt(req);

        let mut cmd = Command::new("gemini");
        cmd.current_dir(&req.working_dir)
            .arg("--prompt")
            .arg(&prompt)
            .arg("--output-format")
            .arg("stream-json");

        // Keep the temp file alive until the process finishes.
        let _policy_file = self.apply_sandbox(&mut cmd)?;

        if let Some(model) = &self.config.model {
            cmd.arg("--model").arg(model);
        }

        if self.config.debug {
            eprintln!(
                "[DEBUG] gemini stream-json (model={})",
                self.config.model.as_deref().unwrap_or("default")
            );
        }

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to run gemini CLI")?;

        let stdout = child.stdout.take().unwrap();
        let stderr_handle = child.stderr.take().unwrap();

        let stderr_task = tokio::spawn(async move {
            let mut buf = String::new();
            let _ = tokio::io::AsyncReadExt::read_to_string(
                &mut BufReader::new(stderr_handle),
                &mut buf,
            )
            .await;
            buf
        });

        let mut reader = BufReader::new(stdout).lines();
        let mut result_text = String::new();

        while let Ok(Some(line)) = reader.next_line().await {
            let event: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };

            super::claude::parse_tool_calls(&event, &events);

            if event.get("type").and_then(|t| t.as_str()) == Some("result")
                && let Some(r) = event.get("result")
            {
                let raw = match r {
                    serde_json::Value::String(s) => s.clone(),
                    _ => r.to_string(),
                };
                result_text = json::extract_json(&raw).unwrap_or(raw);
            }
        }

        let stderr_text = stderr_task.await.unwrap_or_default();
        let status = child.wait().await?;

        if !status.success() {
            anyhow::bail!(ProviderError::BackendFailed(format!(
                "gemini CLI failed (exit {}): {}",
                status,
                stderr_text.trim()
            )));
        }

        if result_text.is_empty() {
            anyhow::bail!(ProviderError::ParseResponse(
                "no result in gemini stream".into()
            ));
        }

        Ok(AiResponse {
            text: result_text,
            usage: None,
        })
    }

    async fn request_batch(&self, req: &AiRequest) -> Result<AiResponse> {
        let prompt = build_prompt(req);

        let mut cmd = Command::new("gemini");
        cmd.current_dir(&req.working_dir)
            .arg("--prompt")
            .arg(&prompt);

        let _policy_file = self.apply_sandbox(&mut cmd)?;

        if let Some(model) = &self.config.model {
            cmd.arg("--model").arg(model);
        }

        if self.config.debug {
            eprintln!(
                "[DEBUG] gemini (model={})",
                self.config.model.as_deref().unwrap_or("default")
            );
        }

        let output = cmd.output().await.context("failed to run gemini CLI")?;
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
                "gemini CLI failed (exit {}): {}",
                output.status,
                stderr.trim()
            )));
        }

        let text = json::extract_json(&raw).unwrap_or(raw);
        Ok(AiResponse { text, usage: None })
    }
}

impl GeminiProvider {
    /// Convert a Sandbox into a Gemini-native TOML policy string.
    fn translate_sandbox_to_toml(&self, sandbox: &Sandbox) -> String {
        let tools = self.translate_sandbox(sandbox);
        if tools.is_empty() {
            return String::new();
        }

        let mut toml = String::new();

        // Process allowed capabilities
        for cap in &sandbox.allowed {
            if sandbox.denied.contains(cap) {
                continue;
            }
            match cap {
                Capability::ReadFile => {
                    toml.push_str(
                        "# Allow file reading, searching, and listing\n\
                         [[rule]]\n\
                         toolName = [\"read_file\", \"glob\", \"grep_search\", \"list_directory\"]\n\
                         decision = \"allow\"\n\
                         priority = 100\n\n",
                    );
                }
                Capability::GitReadOnly => {
                    let cmds: Vec<_> =
                        GIT_READONLY_COMMANDS.iter().map(|c| c.to_string()).collect();
                    let regex = format!("^git ({})", cmds.join("|"));
                    toml.push_str(&format!(
                        "# Allow read-only git commands\n\
                         [[rule]]\n\
                         toolName = \"run_shell_command\"\n\
                         commandRegex = \"{regex}\"\n\
                         decision = \"allow\"\n\
                         priority = 100\n\n",
                    ));
                }
                Capability::ShellCommand { pattern } => {
                    toml.push_str(&format!(
                        "[[rule]]\n\
                         toolName = \"run_shell_command\"\n\
                         commandRegex = \"{pattern}\"\n\
                         decision = \"allow\"\n\
                         priority = 100\n\n",
                    ));
                }
                Capability::Custom(s) => {
                    toml.push_str(s);
                    toml.push('\n');
                }
                Capability::WriteFile | Capability::Network => {}
            }
        }

        // Process denied capabilities
        for cap in &sandbox.denied {
            match cap {
                Capability::WriteFile => {
                    toml.push_str(
                        "# Deny file writing/editing\n\
                         [[rule]]\n\
                         toolName = [\"write_file\", \"replace\"]\n\
                         decision = \"deny\"\n\
                         priority = 50\n\
                         denyMessage = \"Write operations are not permitted.\"\n\n",
                    );
                }
                Capability::ShellCommand { .. } => {
                    toml.push_str(
                        "# Deny shell commands\n\
                         [[rule]]\n\
                         toolName = \"run_shell_command\"\n\
                         decision = \"deny\"\n\
                         priority = 50\n\
                         denyMessage = \"Only read-only git commands are permitted.\"\n\n",
                    );
                }
                Capability::Custom(s) => {
                    toml.push_str(s);
                    toml.push('\n');
                }
                _ => {}
            }
        }

        // Catch-all deny
        toml.push_str(
            "# Deny everything else\n\
             [[rule]]\n\
             toolName = \"*\"\n\
             decision = \"deny\"\n\
             priority = 10\n\
             denyMessage = \"Only read-only operations are allowed.\"\n",
        );

        toml
    }
}

impl SandboxTranslator for GeminiProvider {
    fn translate_allowed(&self, capabilities: &[Capability]) -> Vec<String> {
        // Gemini uses TOML policy, not individual tool entries.
        // Return a non-empty marker so translate_sandbox knows there are restrictions.
        if capabilities.is_empty() {
            Vec::new()
        } else {
            vec!["__gemini_has_policy__".into()]
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn is_available(&self) -> bool {
        Command::new("gemini")
            .arg("--help")
            .output()
            .await
            .is_ok_and(|o| o.status.success())
    }

    async fn request(
        &self,
        req: &AiRequest,
        events: Option<mpsc::UnboundedSender<AiEvent>>,
    ) -> Result<AiResponse> {
        match events {
            Some(tx) => self.request_streaming(req, tx).await,
            None => self.request_batch(req).await,
        }
    }
}

fn build_prompt(req: &AiRequest) -> String {
    let mut prompt = format!("{}\n\n", req.system_prompt);

    if let Some(schema) = &req.json_schema {
        prompt.push_str(&format!(
            "You MUST respond with valid JSON matching this schema:\n```json\n{schema}\n```\n\n\
             Respond ONLY with the JSON object, no markdown fences, no explanation.\n\n"
        ));
    }

    prompt.push_str(&req.user_prompt);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_sr_sandbox_to_toml() {
        let provider = GeminiProvider::new(GeminiConfig::default());
        let sandbox = Sandbox {
            allowed: vec![Capability::GitReadOnly, Capability::ReadFile],
            denied: vec![
                Capability::WriteFile,
                Capability::ShellCommand {
                    pattern: ".*".into(),
                },
            ],
        };
        let toml = provider.translate_sandbox_to_toml(&sandbox);
        assert!(toml.contains("read_file"));
        assert!(toml.contains("commandRegex"));
        assert!(toml.contains("write_file"));
        assert!(toml.contains("toolName = \"*\""));
    }

    #[test]
    fn empty_sandbox_produces_no_policy() {
        let provider = GeminiProvider::new(GeminiConfig::default());
        let sandbox = Sandbox::default();
        let toml = provider.translate_sandbox_to_toml(&sandbox);
        assert!(toml.is_empty());
    }
}
