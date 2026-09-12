//! Render portable agent metadata at the tool boundary, never in the shared store.
use crate::{
    error::{AppError, Result},
    frontmatter,
};
use serde_yaml_ng::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn render(source: &str, tool: &str) -> Result<String> {
    let parsed = frontmatter::parse(source)?;
    let mut fm: Mapping = serde_yaml_ng::from_str(&parsed.frontmatter)?;
    if tool != "opencode" && !fm.contains_key("models") {
        return Ok(source.to_owned());
    }
    // Optional exact per-tool IDs, including proxy prefixes. Never guess a provider.
    if let Some(models) = fm.remove("models") {
        let models = models
            .as_mapping()
            .ok_or_else(|| AppError::Other("agent models must be a mapping".into()))?;
        if let Some(model) = models.get(tool) {
            if !model.is_string() {
                return Err(AppError::Other(
                    "agent model override must be a string".into(),
                ));
            }
            fm.insert("model".into(), model.clone());
        }
    }
    if tool == "claude-code" && fm.get("model").is_some_and(|v| !v.is_string()) {
        return Err(AppError::Other(
            "Claude Code requires one model string; set models.claude-code".into(),
        ));
    }
    if tool == "opencode" {
        if fm.get("model").is_some_and(|v| !v.is_string()) {
            return Err(AppError::Other("OpenCode requires one model string, not a vendor fallback list; set models.opencode".into()));
        }
        if let Some(model) = fm.get("model").and_then(Value::as_str) {
            if matches!(model, "inherit" | "sonnet" | "haiku" | "opus") {
                // Omission inherits the calling agent's configured provider and model.
                fm.remove("model");
            } else if !model.contains('/') {
                return Err(AppError::Other(format!(
                    "OpenCode model '{model}' needs provider/model; set models.opencode or use inherit"
                )));
            }
        }
        fm.remove("name");
        fm.entry(Value::from("mode"))
            .or_insert(Value::from("subagent"));
        if let Some(tools) = fm.remove("tools") {
            if let Some(list) = tools.as_str() {
                if fm.contains_key("permission") {
                    return Err(AppError::Other(
                        "agent mixes Claude tools and OpenCode permission; translate explicitly"
                            .into(),
                    ));
                }
                let mut permission = Mapping::new();
                permission.insert("*".into(), "deny".into());
                for entry in list.split(',').map(str::trim) {
                    let (name, pattern) = entry
                        .split_once('(')
                        .map_or((entry, None), |(n, p)| (n, p.strip_suffix(')')));
                    let native = match name {
                        "Read" => "read",
                        "Edit" | "Write" | "MultiEdit" => "edit",
                        "Grep" => "grep",
                        "Glob" => "glob",
                        "Bash" => "bash",
                        "WebFetch" => "webfetch",
                        "WebSearch" => "websearch",
                        "Task" | "Agent" => "task",
                        "Skill" => "skill",
                        "TodoWrite" => "todowrite",
                        _ => {
                            return Err(AppError::Other(format!(
                                "unsupported agent tool '{entry}' for OpenCode"
                            )));
                        }
                    };
                    if let Some(pattern) = pattern {
                        if native != "bash" {
                            return Err(AppError::Other(format!(
                                "unsupported scoped tool '{entry}'"
                            )));
                        }
                        let rules = permission.entry(Value::from(native)).or_insert_with(|| {
                            let mut rules = Mapping::new();
                            rules.insert("*".into(), "deny".into());
                            Value::Mapping(rules)
                        });
                        if let Some(rules) = rules.as_mapping_mut() {
                            rules.insert(pattern.into(), "allow".into());
                        }
                    } else {
                        permission.insert(native.into(), "allow".into());
                    }
                }
                fm.insert("permission".into(), Value::Mapping(permission));
            } else if tools.is_mapping() {
                fm.insert("tools".into(), tools);
            } else {
                return Err(AppError::Other("unsupported agent tools format".into()));
            }
        }
    }
    Ok(frontmatter::compose(
        &serde_yaml_ng::to_string(&fm)?,
        &parsed.body,
    ))
}

pub fn write(source: &Path, target: &Path, tool: &str) -> Result<()> {
    let rendered = render(&std::fs::read_to_string(source)?, tool)?;
    // Never follow a legacy symlink back into the canonical source.
    if target.is_symlink() {
        std::fs::remove_file(target)?;
    }
    std::fs::write(target, rendered)?;
    Ok(())
}

pub fn expected_hash(source: &Path, tool: &str) -> Result<String> {
    let rendered = render(&std::fs::read_to_string(source)?, tool)?;
    Ok(format!("sha256:{:x}", Sha256::digest(rendered.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_native_files_keep_their_hashes() {
        let source = "---\nname: native\nmodel: sonnet\n---\nBody\n";
        assert_eq!(render(source, "claude-code").unwrap(), source);
        assert!(render("---\nmodel: [first, second]\n---\n", "opencode").is_err());
    }
    #[test]
    fn inherit_and_scoped_permissions() {
        let text = render(
            "---\nname: test\nmodel: sonnet\ntools: Read, Bash(git *), Bash(gh *)\n---\nBody",
            "opencode",
        )
        .unwrap();
        let fm: Value =
            serde_yaml_ng::from_str(&frontmatter::parse(&text).unwrap().frontmatter).unwrap();
        assert!(fm.get("model").is_none());
        assert_eq!(fm["mode"], "subagent");
        assert_eq!(fm["permission"]["*"], "deny");
        assert_eq!(fm["permission"]["bash"]["git *"], "allow");
        assert_eq!(fm["permission"]["bash"]["gh *"], "allow");
        assert!(text.ends_with("Body"));
    }
    #[test]
    fn exact_proxy_overrides_and_claude_aliases() {
        let src = "---\nmodel: sonnet\nmodels:\n  opencode: proxy/anthropic/claude-sonnet-5\n  claude-code: anthropic/claude-sonnet-5\n---\nBody";
        for (tool, model) in [
            ("opencode", "proxy/anthropic/claude-sonnet-5"),
            ("claude-code", "anthropic/claude-sonnet-5"),
        ] {
            let out = render(src, tool).unwrap();
            let fm: Value =
                serde_yaml_ng::from_str(&frontmatter::parse(&out).unwrap().frontmatter).unwrap();
            assert_eq!(fm["model"], model);
            assert!(fm.get("models").is_none());
        }
        assert!(render("---\nmodel: unknown-model\n---\n", "opencode").is_err());
    }
}
