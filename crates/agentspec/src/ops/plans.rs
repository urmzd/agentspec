//! Planning artifact sync.
//!
//! Plans are portable markdown documents stored in the canonical
//! `~/.agents/plans/` store. They become first-class managed resources
//! (`ResourceKind::Plan`) discoverable by `status`/`manage list`.
//!
//! `plans import` pulls Gemini CLI ("antigravity") planning artifacts —
//! task definitions, implementation plans, and walkthroughs stored under
//! `~/.gemini/antigravity/brain/<session>/` — into the canonical store as
//! portable markdown with YAML frontmatter preserving their metadata.

use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::frontmatter;
use crate::inventory::{Config, SourceType, TrackedKind, TrackedResource, hash_resource};

/// The three Gemini antigravity artifact kinds, by their on-disk base name.
const GEMINI_ARTIFACTS: &[&str] = &["task", "implementation_plan", "walkthrough"];

struct ArtifactMeta {
    summary: String,
    updated_at: String,
    artifact_type: String,
}

fn read_meta(meta_path: &Path, fallback_type: &str) -> ArtifactMeta {
    let default = ArtifactMeta {
        summary: String::new(),
        updated_at: String::new(),
        artifact_type: fallback_type.to_string(),
    };
    let Ok(text) = std::fs::read_to_string(meta_path) else {
        return default;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return default;
    };
    ArtifactMeta {
        summary: val
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        updated_at: val
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        artifact_type: val
            .get("artifactType")
            .and_then(|v| v.as_str())
            .unwrap_or(fallback_type)
            .to_string(),
    }
}

/// Render a canonical plan markdown document with frontmatter.
fn render_plan(name: &str, meta: &ArtifactMeta, session: &str, body: &str) -> String {
    let description = if meta.summary.is_empty() {
        format!("Imported {} plan", meta.artifact_type)
    } else {
        meta.summary.clone()
    };
    // Build frontmatter via serde_yaml_ng so values containing `:`, `#`, newlines,
    // or a stray `---` are quoted/escaped rather than producing malformed YAML.
    let mut fm = serde_yaml_ng::Mapping::new();
    fm.insert("name".into(), name.into());
    fm.insert("description".into(), description.into());
    fm.insert("source".into(), "gemini-antigravity".into());
    fm.insert("session".into(), session.into());
    fm.insert("artifact_type".into(), meta.artifact_type.as_str().into());
    if !meta.updated_at.is_empty() {
        fm.insert("updated_at".into(), meta.updated_at.as_str().into());
    }
    let yaml = serde_yaml_ng::to_string(&serde_yaml_ng::Value::Mapping(fm)).unwrap_or_default();

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&yaml);
    out.push_str("---\n\n");
    out.push_str(body.trim_end());
    out.push('\n');
    out
}

/// Import Gemini antigravity planning artifacts into the canonical plan store.
pub fn import_gemini(cfg: &mut Config, json: bool) -> Result<()> {
    let brain = config::home_dir()
        .join(".gemini")
        .join("antigravity")
        .join("brain");
    if !brain.exists() {
        return Err(AppError::Other(format!(
            "Gemini antigravity brain directory not found: {}",
            brain.display()
        )));
    }

    let dest_dir = config::shared_plans_dir();
    std::fs::create_dir_all(&dest_dir)?;

    let mut imported: Vec<String> = Vec::new();

    for session_entry in std::fs::read_dir(&brain)?.filter_map(|e| e.ok()) {
        let sdir = session_entry.path();
        if !sdir.is_dir() {
            continue;
        }
        let uuid = sdir.file_name().unwrap().to_string_lossy().to_string();

        for base in GEMINI_ARTIFACTS {
            let md = sdir.join(format!("{base}.md"));
            if !md.exists() {
                continue;
            }
            // Prefer the `.resolved` view (file:// links + placeholders expanded).
            let resolved = sdir.join(format!("{base}.md.resolved"));
            let content_path = if resolved.exists() { &resolved } else { &md };
            let Ok(body) = std::fs::read_to_string(content_path) else {
                continue;
            };

            let meta_path = sdir.join(format!("{base}.md.metadata.json"));
            let meta = read_meta(&meta_path, base);

            // Full UUID keeps plan names collision-free across sessions.
            let name = format!("{}-{}", base.replace('_', "-"), uuid);
            let plan_md = render_plan(&name, &meta, &uuid, &body);

            let dest = dest_dir.join(format!("{name}.md"));
            std::fs::write(&dest, &plan_md)?;

            let hash = hash_resource(TrackedKind::Plan, &dest)?;
            let tracked = TrackedResource::new(
                name.clone(),
                TrackedKind::Plan,
                md.to_string_lossy().to_string(),
                SourceType::Local,
                format!("plans/{name}.md"),
                hash,
            );
            cfg.add(tracked);
            imported.push(name);
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&imported)?);
    } else if imported.is_empty() {
        println!(
            "  {} no Gemini planning artifacts found",
            style("~").yellow()
        );
    } else {
        for name in &imported {
            println!("  {} imported plan '{name}'", style("✓").green().bold());
        }
        println!(
            "\n  {} plan(s) imported into {}",
            imported.len(),
            dest_dir.display()
        );
    }

    Ok(())
}

/// List plans in the canonical store.
pub fn list_plans(json: bool) -> Result<()> {
    let dir = config::shared_plans_dir();
    let mut plans: Vec<(String, String)> = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let description = read_plan_description(&path).unwrap_or_default();
            plans.push((name, description));
        }
    }
    plans.sort();

    if json {
        let arr: Vec<serde_json::Value> = plans
            .iter()
            .map(|(n, d)| serde_json::json!({ "name": n, "description": d }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
    } else if plans.is_empty() {
        println!("  no plans in {}", dir.display());
    } else {
        for (name, desc) in &plans {
            println!("  {} {:<32} {}", style("●").green(), name, desc);
        }
    }

    Ok(())
}

fn read_plan_description(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = frontmatter::parse(&content).ok()?;
    let fm: serde_yaml_ng::Value = serde_yaml_ng::from_str(&parsed.frontmatter).ok()?;
    fm.get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fm(md: &str) -> serde_yaml_ng::Mapping {
        let parsed = frontmatter::parse(md).unwrap();
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&parsed.frontmatter)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone()
    }

    fn fm_str(fm: &serde_yaml_ng::Mapping, key: &str) -> Option<String> {
        fm.get(serde_yaml_ng::Value::String(key.into()))
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    #[test]
    fn renders_plan_with_frontmatter() {
        let meta = ArtifactMeta {
            summary: "Do the thing".into(),
            updated_at: "2026-01-17T22:04:29Z".into(),
            artifact_type: "ARTIFACT_TYPE_WALKTHROUGH".into(),
        };
        let md = render_plan(
            "walkthrough-0c1efb0f",
            &meta,
            "0c1efb0f-uuid",
            "# Body\n\ntext",
        );
        assert!(md.starts_with("---\n"));
        let fm = parse_fm(&md);
        assert_eq!(fm_str(&fm, "name").unwrap(), "walkthrough-0c1efb0f");
        assert_eq!(fm_str(&fm, "description").unwrap(), "Do the thing");
        assert_eq!(fm_str(&fm, "source").unwrap(), "gemini-antigravity");
        assert_eq!(fm_str(&fm, "session").unwrap(), "0c1efb0f-uuid");
        assert_eq!(
            fm_str(&fm, "artifact_type").unwrap(),
            "ARTIFACT_TYPE_WALKTHROUGH"
        );
        assert_eq!(fm_str(&fm, "updated_at").unwrap(), "2026-01-17T22:04:29Z");
        assert!(md.trim_end().ends_with("# Body\n\ntext"));
    }

    #[test]
    fn frontmatter_is_safe_against_yaml_injection() {
        let meta = ArtifactMeta {
            // A summary that would break naive frontmatter: colon, hash, and a
            // line that looks like a frontmatter terminator.
            summary: "fix: thing #1\n---\nnot: frontmatter".into(),
            updated_at: String::new(),
            artifact_type: "task".into(),
        };
        let md = render_plan("task-x", &meta, "uuid", "body");
        let fm = parse_fm(&md);
        // The whole summary survives as a single string value.
        assert_eq!(
            fm_str(&fm, "description").unwrap(),
            "fix: thing #1\n---\nnot: frontmatter"
        );
    }

    #[test]
    fn render_plan_defaults_description_when_no_summary() {
        let meta = ArtifactMeta {
            summary: String::new(),
            updated_at: String::new(),
            artifact_type: "task".into(),
        };
        let md = render_plan("task-abc", &meta, "abc", "body");
        let fm = parse_fm(&md);
        assert_eq!(fm_str(&fm, "description").unwrap(), "Imported task plan");
        assert!(
            fm.get(serde_yaml_ng::Value::String("updated_at".into()))
                .is_none()
        );
    }

    #[test]
    fn reads_metadata_json() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("m.json");
        std::fs::write(
            &p,
            r#"{"artifactType":"ARTIFACT_TYPE_TASK","summary":"S","updatedAt":"T"}"#,
        )
        .unwrap();
        let meta = read_meta(&p, "fallback");
        assert_eq!(meta.summary, "S");
        assert_eq!(meta.updated_at, "T");
        assert_eq!(meta.artifact_type, "ARTIFACT_TYPE_TASK");

        // Missing file → fallback.
        let meta2 = read_meta(&tmp.path().join("nope.json"), "fallback");
        assert_eq!(meta2.artifact_type, "fallback");
        assert!(meta2.summary.is_empty());
    }
}
