use crate::error::ProviderError;
use crate::ir::{Capability, GIT_READONLY_COMMANDS, ProviderConfig, Sandbox, SandboxTranslator};
use crate::types::{AiEvent, AiProvider, AiRequest, AiResponse, AiUsage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use super::json::embed_schema;

/// Configuration for the Claude CLI provider.
pub(crate) struct ClaudeConfig {
    pub model: Option<String>,
    pub budget: f64,
    pub sandbox: Option<Sandbox>,
    pub debug: bool,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            model: None,
            budget: 0.50,
            sandbox: None,
            debug: false,
        }
    }
}

pub struct ClaudeProvider {
    config: ClaudeConfig,
}

impl ClaudeProvider {
    pub(crate) fn new(config: ClaudeConfig) -> Self {
        Self { config }
    }

    pub(crate) fn from_provider_config(config: &ProviderConfig) -> Self {
        Self::new(ClaudeConfig {
            model: config.model.clone(),
            budget: config.budget.unwrap_or(0.50),
            sandbox: config.sandbox.clone(),
            debug: config.debug,
        })
    }

    fn base_command(&self, working_dir: &str) -> Command {
        let model = self.config.model.as_deref().unwrap_or("haiku");
        let mut cmd = Command::new("claude");
        cmd.current_dir(working_dir).arg("--model").arg(model);

        if let Some(sandbox) = &self.config.sandbox {
            for tool in self.translate_sandbox(sandbox) {
                cmd.arg("--allowed-tools").arg(tool);
            }
        }

        cmd.arg("--max-budget-usd")
            .arg(format!("{:.2}", self.config.budget))
            .arg("-p");
        cmd
    }

    async fn request_streaming(
        &self,
        req: &AiRequest,
        events: UnboundedSender<AiEvent>,
    ) -> Result<AiResponse> {
        let system = embed_schema(&req.system_prompt, req.json_schema.as_deref());

        let mut cmd = self.base_command(&req.working_dir);
        cmd.arg(&req.user_prompt)
            .arg("--system-prompt")
            .arg(&system)
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose");

        if self.config.debug {
            eprintln!(
                "[DEBUG] claude stream-json (model={}, budget={:.2})",
                self.config.model.as_deref().unwrap_or("haiku"),
                self.config.budget
            );
        }

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to run claude CLI")?;

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
        let mut usage = None;

        while let Ok(Some(line)) = reader.next_line().await {
            let event: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };

            parse_tool_calls(&event, &events);

            if event.get("type").and_then(|t| t.as_str()) == Some("result") {
                if let Some(r) = event.get("result") {
                    let raw = match r {
                        serde_json::Value::String(s) => s.clone(),
                        _ => r.to_string(),
                    };
                    result_text = super::json::extract_json(&raw).unwrap_or(raw);
                }
                usage = extract_usage(&event);
            }
        }

        let stderr_text = stderr_task.await.unwrap_or_default();
        let status = child.wait().await?;

        if !status.success() {
            anyhow::bail!(ProviderError::BackendFailed(format!(
                "claude CLI failed (exit {}): {}",
                status,
                stderr_text.trim()
            )));
        }

        if result_text.is_empty() {
            anyhow::bail!(ProviderError::ParseResponse(
                "no result in claude stream".into()
            ));
        }

        Ok(AiResponse {
            text: result_text,
            usage,
        })
    }

    async fn request_batch(&self, req: &AiRequest) -> Result<AiResponse> {
        let mut cmd = self.base_command(&req.working_dir);
        cmd.arg(&req.user_prompt)
            .arg("--system-prompt")
            .arg(&req.system_prompt)
            .arg("--output-format")
            .arg("json");

        if let Some(schema) = &req.json_schema {
            cmd.arg("--json-schema").arg(schema);
        }

        if self.config.debug {
            eprintln!(
                "[DEBUG] claude json (model={}, budget={:.2})",
                self.config.model.as_deref().unwrap_or("haiku"),
                self.config.budget
            );
        }

        let output = cmd.output().await.context("failed to run claude CLI")?;
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
                "claude CLI failed (exit {}): {}",
                output.status,
                stderr.trim()
            )));
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&raw).context("failed to parse claude JSON response")?;
        let usage = extract_usage(&parsed);

        if req.json_schema.is_some() {
            let structured = &parsed["structured_output"];
            if structured.is_null() {
                anyhow::bail!(ProviderError::ParseResponse(
                    "empty structured_output from claude".into()
                ));
            }
            Ok(AiResponse {
                text: structured.to_string(),
                usage,
            })
        } else {
            let text = parsed
                .get("result")
                .map(|r| match r {
                    serde_json::Value::String(s) => s.clone(),
                    _ => r.to_string(),
                })
                .unwrap_or(raw);
            Ok(AiResponse { text, usage })
        }
    }
}

impl SandboxTranslator for ClaudeProvider {
    fn translate_allowed(&self, capabilities: &[Capability]) -> Vec<String> {
        let mut tools = Vec::new();
        for cap in capabilities {
            match cap {
                Capability::ReadFile => tools.push("Read".into()),
                Capability::GitReadOnly => {
                    for cmd in GIT_READONLY_COMMANDS {
                        tools.push(format!("Bash(git:{cmd})"));
                    }
                }
                Capability::ShellCommand { pattern } => {
                    tools.push(format!("Bash({pattern})"));
                }
                Capability::Custom(s) => tools.push(s.clone()),
                Capability::WriteFile | Capability::Network => {}
            }
        }
        tools
    }
}

#[async_trait]
impl AiProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    async fn is_available(&self) -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .await
            .is_ok_and(|o| o.status.success())
    }

    async fn request(
        &self,
        req: &AiRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AiEvent>>,
    ) -> Result<AiResponse> {
        match events {
            Some(tx) => self.request_streaming(req, tx).await,
            None => self.request_batch(req).await,
        }
    }
}

/// Parse tool calls from Claude's NDJSON stream events.
pub(crate) fn parse_tool_calls(event: &serde_json::Value, events: &UnboundedSender<AiEvent>) {
    if let Some(content) = event.pointer("/message/content")
        && let Some(arr) = content.as_array()
    {
        for item in arr {
            if item["type"] == "tool_use"
                && let Some(input) = extract_tool_input(item)
            {
                let tool = item["name"].as_str().unwrap_or("unknown").to_string();
                let _ = events.send(AiEvent::ToolCall { tool, input });
            }
        }
    }

    if event.get("type").and_then(|t| t.as_str()) == Some("stream_event")
        && let Some(inner) = event.get("event")
        && inner.get("type").and_then(|t| t.as_str()) == Some("content_block_start")
        && let Some(block) = inner.get("content_block")
        && block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
    {
        let tool = block["name"].as_str().unwrap_or("unknown").to_string();
        let input = extract_tool_input(block).unwrap_or_default();
        if !input.is_empty() {
            let _ = events.send(AiEvent::ToolCall { tool, input });
        }
    }
}

fn extract_tool_input(item: &serde_json::Value) -> Option<String> {
    if let Some(cmd) = item.pointer("/input/command").and_then(|c| c.as_str()) {
        return Some(cmd.to_string());
    }
    if let Some(path) = item.pointer("/input/file_path").and_then(|p| p.as_str()) {
        return Some(path.to_string());
    }
    item.get("input")
        .filter(|i| !i.is_null())
        .map(|i| serde_json::to_string(i).unwrap_or_default())
        .filter(|s| !s.is_empty() && s != "{}")
}

fn extract_usage(parsed: &serde_json::Value) -> Option<AiUsage> {
    let u = parsed.get("usage")?;
    Some(AiUsage {
        input_tokens: u.get("input_tokens")?.as_u64()?,
        output_tokens: u.get("output_tokens")?.as_u64()?,
        cost_usd: parsed.get("cost_usd").and_then(|c| c.as_f64()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_git_readonly() {
        let provider = ClaudeProvider::new(ClaudeConfig::default());
        let caps = vec![Capability::GitReadOnly];
        let tools = provider.translate_allowed(&caps);
        assert!(tools.contains(&"Bash(git:diff)".to_string()));
        assert!(tools.contains(&"Bash(git:log)".to_string()));
        assert!(tools.contains(&"Bash(git:blame)".to_string()));
        assert_eq!(tools.len(), GIT_READONLY_COMMANDS.len());
    }

    #[test]
    fn translate_read_file() {
        let provider = ClaudeProvider::new(ClaudeConfig::default());
        let caps = vec![Capability::ReadFile];
        let tools = provider.translate_allowed(&caps);
        assert_eq!(tools, vec!["Read"]);
    }

    #[test]
    fn translate_custom_passthrough() {
        let provider = ClaudeProvider::new(ClaudeConfig::default());
        let caps = vec![Capability::Custom("Bash(npm:test)".into())];
        let tools = provider.translate_allowed(&caps);
        assert_eq!(tools, vec!["Bash(npm:test)"]);
    }

    #[test]
    fn translate_sandbox_filters_denied() {
        let provider = ClaudeProvider::new(ClaudeConfig::default());
        let sandbox = Sandbox {
            allowed: vec![Capability::GitReadOnly, Capability::ReadFile],
            denied: vec![Capability::ReadFile],
        };
        let tools = provider.translate_sandbox(&sandbox);
        assert!(!tools.contains(&"Read".to_string()));
        assert!(tools.contains(&"Bash(git:diff)".to_string()));
    }

    #[test]
    fn translate_empty_sandbox() {
        let provider = ClaudeProvider::new(ClaudeConfig::default());
        let sandbox = Sandbox::default();
        let tools = provider.translate_sandbox(&sandbox);
        assert!(tools.is_empty());
    }
}
