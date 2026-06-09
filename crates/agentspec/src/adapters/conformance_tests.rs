//! Vendor-format conformance suite for the resource adapters.
//!
//! Golden snapshots pin what each fixture parses to, and the round-trip
//! invariant `parse(emit(parse(f))) == parse(f)` guards against data loss
//! when a resource is rewritten in its vendor format.

use std::path::{Path, PathBuf};

use proptest::prelude::*;

use super::*;
use crate::ir::Resource;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/resources")
        .join(rel)
}

/// Every resource fixture paired with the adapter that owns its format.
fn fixture_corpus() -> Vec<(&'static str, Box<dyn Adapter>)> {
    vec![
        (
            "skill-basic/SKILL.md",
            Box::new(agentskills::AgentSkillsAdapter),
        ),
        (
            "skill-full/SKILL.md",
            Box::new(agentskills::AgentSkillsAdapter),
        ),
        ("agents/code-reviewer.md", Box::new(claude::ClaudeAdapter)),
        ("agents/full-agent.md", Box::new(claude::ClaudeAdapter)),
        (
            "gemini-agents/researcher.md",
            Box::new(gemini::GeminiAdapter),
        ),
        ("project/AGENTS.md", Box::new(agents_md::AgentsMdAdapter)),
        ("project/CLAUDE.md", Box::new(claude_md::ClaudeMdAdapter)),
        ("project/llms.txt", Box::new(llms_txt::LlmsTxtAdapter)),
        (
            "instructions/GEMINI.md",
            Box::new(instruction_file::InstructionFileAdapter),
        ),
        (
            "instructions/.cursorrules",
            Box::new(instruction_file::InstructionFileAdapter),
        ),
    ]
}

#[test]
fn fixtures_match_golden_snapshots() {
    for (rel, adapter) in fixture_corpus() {
        let resource = adapter
            .parse(&fixture(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let name = rel.replace('/', "__");
        insta::with_settings!({sort_maps => true, snapshot_path => "../../tests/snapshots"}, {
            insta::assert_yaml_snapshot!(name, resource);
        });
    }
}

#[test]
fn round_trip_parse_emit_parse_is_identity() {
    let tmp = tempfile::tempdir().unwrap();
    for (rel, adapter) in fixture_corpus() {
        let path = fixture(rel);
        let first = adapter
            .parse(&path)
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let emitted = adapter.emit(&first).unwrap();
        // Same filename: the instruction-file adapter derives names from it.
        let reparse_path = tmp.path().join(path.file_name().unwrap());
        std::fs::write(&reparse_path, &emitted).unwrap();
        let second = adapter.parse(&reparse_path).unwrap_or_else(|e| {
            panic!("{rel}: emitted output failed to parse: {e}\n---\n{emitted}")
        });
        assert_eq!(
            first, second,
            "{rel}: parse(emit(parse(f))) != parse(f)\nemitted:\n{emitted}"
        );
    }
}

#[test]
fn unknown_frontmatter_keys_survive_into_metadata() {
    let skill = agentskills::AgentSkillsAdapter
        .parse(&fixture("skill-full/SKILL.md"))
        .unwrap();
    assert_eq!(
        skill.metadata.get("customVendorField"),
        Some(&serde_yaml_ng::Value::Number(1.into()))
    );
    assert_eq!(
        skill.metadata.get("author"),
        Some(&serde_yaml_ng::Value::String("urmzd".into()))
    );

    let agent = claude::ClaudeAdapter
        .parse(&fixture("agents/full-agent.md"))
        .unwrap();
    assert_eq!(
        agent.metadata.get("customVendorField"),
        Some(&serde_yaml_ng::Value::Number(1.into()))
    );
    let ext = agent.extensions.get("claude-code").unwrap();
    assert!(ext.get("hooks").is_some());
    assert!(ext.get("mcpServers").is_some());
    assert!(ext.get("memory").is_some());

    let researcher = gemini::GeminiAdapter
        .parse(&fixture("gemini-agents/researcher.md"))
        .unwrap();
    assert_eq!(
        researcher.metadata.get("customGeminiField"),
        Some(&serde_yaml_ng::Value::String("experimental".into()))
    );
    let ext = researcher.extensions.get("gemini-cli").unwrap();
    assert_eq!(
        ext.get("kind"),
        Some(&serde_yaml_ng::Value::String("local".into()))
    );
}

#[test]
fn adapter_for_path_routes_by_filename_and_context() {
    let cases = [
        ("/p/SKILL.md", Some("agentskills")),
        ("/p/AGENTS.md", Some("agents-md")),
        ("/p/CLAUDE.md", Some("claude-md")),
        ("/p/llms.txt", Some("llms-txt")),
        ("/p/GEMINI.md", Some("instruction-file")),
        ("/p/.cursorrules", Some("instruction-file")),
        ("/p/.clinerules", Some("instruction-file")),
        ("/p/.windsurfrules", Some("instruction-file")),
        ("/p/copilot-instructions.md", Some("instruction-file")),
        ("/p/codex-instructions.md", Some("instruction-file")),
        ("/home/u/.claude/agents/helper.md", Some("claude-code")),
        ("/home/u/.agents/agents/helper.md", Some("claude-code")),
        ("/home/u/.gemini/agents/helper.md", Some("gemini-cli")),
        ("/p/notes.md", Some("claude-code")),
        ("/p/notes.txt", None),
    ];
    for (path, expected) in cases {
        let vendor = adapter_for_path(Path::new(path)).map(|a| a.vendor().to_string());
        assert_eq!(vendor.as_deref(), expected, "routing for {path}");
    }
}

fn write_and_parse(dir: &Path, doc: &str) -> Resource {
    let path = dir.join("agent.md");
    std::fs::write(&path, doc).unwrap();
    claude::ClaudeAdapter.parse(&path).unwrap()
}

proptest! {
    /// Claude `tools:` accepts a comma string or a YAML list; both spellings
    /// must normalize to the same IR.
    #[test]
    fn comma_string_and_yaml_list_tools_produce_identical_ir(
        tokens in proptest::collection::vec("[A-Za-z][A-Za-z0-9_-]{0,11}", 1..6),
        pads in proptest::collection::vec((0usize..3, 0usize..3), 6),
        trailing_comma in proptest::bool::ANY,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let padded: Vec<String> = tokens
            .iter()
            .zip(pads.iter().cycle())
            .map(|(t, (l, r))| format!("{}{}{}", " ".repeat(*l), t, " ".repeat(*r)))
            .collect();
        let mut comma = padded.join(",");
        if trailing_comma {
            comma.push(',');
        }
        let string_doc =
            format!("---\nname: t\ndescription: d\ntools: \"{comma}\"\n---\n\nbody\n");
        let list_doc = format!(
            "---\nname: t\ndescription: d\ntools:\n{}---\n\nbody\n",
            tokens
                .iter()
                .map(|t| format!("  - \"{t}\"\n"))
                .collect::<String>()
        );

        let from_string = write_and_parse(tmp.path(), &string_doc);
        let from_list = write_and_parse(tmp.path(), &list_doc);
        prop_assert_eq!(&from_string, &from_list);
        prop_assert_eq!(from_string.tools.as_deref(), Some(tokens.as_slice()));
    }
}
