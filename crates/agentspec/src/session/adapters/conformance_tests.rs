//! Vendor-format conformance suite for the session parsers.
//!
//! Each fixture is an anonymized session in a vendor's on-disk format;
//! the golden snapshots pin the SessionIR it parses to, so any parser
//! change shows up as a reviewable diff.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::session::ir::RoleIR;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sessions")
        .join(rel)
}

macro_rules! assert_session_snapshot {
    ($name:expr, $session:expr) => {
        insta::with_settings!({sort_maps => true, snapshot_path => "../../../tests/snapshots"}, {
            insta::assert_yaml_snapshot!($name, $session);
        });
    };
}

#[test]
fn claude_fixtures_match_golden_snapshots() {
    for name in ["happy", "tools", "meta", "malformed", "empty"] {
        let session = claude::parse_session_file(&fixture(&format!("claude/{name}.jsonl")))
            .unwrap_or_else(|e| panic!("claude/{name}: {e}"));
        assert_session_snapshot!(format!("claude__{name}"), session);
    }
}

#[test]
fn claude_meta_lines_are_excluded_from_first_prompt() {
    let session = claude::parse_session_file(&fixture("claude/meta.jsonl")).unwrap();
    assert_eq!(
        session.first_prompt.as_deref(),
        Some("Real first prompt after the meta lines")
    );
}

#[test]
fn claude_empty_file_parses_to_empty_session() {
    let session = claude::parse_session_file(&fixture("claude/empty.jsonl")).unwrap();
    assert_eq!(session.id, "empty");
    assert!(session.messages.is_empty());
    assert!(session.started_at.is_none());
}

#[test]
fn claude_list_sessions_filters_jsonl_and_sorts_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_a = tmp.path().join("-home-user-a");
    let proj_b = tmp.path().join("-home-user-b");
    fs::create_dir_all(&proj_a).unwrap();
    fs::create_dir_all(&proj_b).unwrap();

    let line = |ts: &str| {
        format!(
            r#"{{"type":"user","cwd":"/home/user/a","timestamp":"{ts}","message":{{"content":"hi"}}}}"#
        )
    };
    fs::write(proj_a.join("older.jsonl"), line("2026-01-01T00:00:00Z")).unwrap();
    fs::write(proj_b.join("newer.jsonl"), line("2026-02-01T00:00:00Z")).unwrap();
    fs::write(proj_a.join("notes.txt"), "not a session").unwrap();
    fs::write(tmp.path().join("stray.jsonl"), line("2026-03-01T00:00:00Z")).unwrap();

    let sessions = claude::list_sessions_in_root(tmp.path()).unwrap();
    let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["newer", "older"]);
}

#[test]
fn codex_fixtures_match_golden_snapshots() {
    let cases = [
        ("happy.jsonl", "codex__happy"),
        (
            "rollout-2026-01-03T11-00-00-9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4.jsonl",
            "codex__rollout",
        ),
        ("malformed.jsonl", "codex__malformed"),
    ];
    for (file, name) in cases {
        let session = codex::parse_session_file(&fixture(&format!("codex/{file}")))
            .unwrap_or_else(|e| panic!("codex/{file}: {e}"));
        assert_session_snapshot!(name, session);
    }
}

#[test]
fn codex_filters_system_context_from_messages_and_first_prompt() {
    let session = codex::parse_session_file(&fixture(
        "codex/rollout-2026-01-03T11-00-00-9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4.jsonl",
    ))
    .unwrap();
    assert!(
        session.messages.iter().all(|m| m.role != RoleIR::System),
        "system/developer messages must be filtered"
    );
    assert_eq!(
        session.first_prompt.as_deref(),
        Some("Run the test suite and fix failures")
    );
    assert_eq!(session.tools_used, ["shell"]);
}

#[test]
fn codex_session_id_comes_from_rollout_filename() {
    let path =
        fixture("codex/rollout-2026-01-03T11-00-00-9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4.jsonl");
    assert_eq!(
        codex::extract_session_id_from_filename(&path),
        "9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4"
    );
    // Non-rollout filenames fall back to the stem.
    assert_eq!(
        codex::extract_session_id_from_filename(Path::new("/x/happy.jsonl")),
        "happy"
    );
}

#[test]
fn copilot_fixture_matches_golden_snapshot() {
    // enrich_from_db is a no-op here: the fixture UUID never exists in a
    // real ~/.copilot/session-store.db, and the open is read-only.
    let session =
        copilot::parse_session(&fixture("copilot/0f0f0f0f-aaaa-bbbb-cccc-121212121212")).unwrap();
    assert_session_snapshot!("copilot__happy", session);
}

#[test]
fn gemini_fixtures_match_golden_snapshots() {
    // The fixture project name must not exist under ~/.gemini/history so
    // the cwd lookup stays None on every machine.
    let session = gemini::parse_session(
        &fixture("gemini/session-2026-01-05T08-00-00.json"),
        "agentspec-fixture-project",
    )
    .unwrap();
    assert_session_snapshot!("gemini__happy", session);

    let empty = gemini::parse_session(
        &fixture("gemini/session-empty.json"),
        "agentspec-fixture-project",
    )
    .unwrap();
    assert!(empty.messages.is_empty());
    assert_session_snapshot!("gemini__empty", empty);
}

#[test]
fn gemini_malformed_json_is_an_error() {
    let result = gemini::parse_session(
        &fixture("gemini/session-malformed.json"),
        "agentspec-fixture-project",
    );
    assert!(result.is_err());
}
