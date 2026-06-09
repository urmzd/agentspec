//! CLI integration tests pinning the `--format json` contract.
//!
//! Every json-emitting subcommand must write exactly one JSON document to
//! stdout — human progress lines belong on stderr or behind the human format.
//! Each test runs against a throwaway HOME so no real user state is touched.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

fn agentspec(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agentspec").unwrap();
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(home);
    cmd
}

/// Parse stdout as a single JSON document. Fails when human output leaks in.
fn stdout_json(stdout: &[u8]) -> serde_json::Value {
    let s = std::str::from_utf8(stdout).expect("stdout is not UTF-8");
    serde_json::from_str(s)
        .unwrap_or_else(|e| panic!("stdout is not a single JSON document: {e}\n---\n{s}"))
}

#[test]
fn status_json_is_single_document() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "status", "--fast"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["managed"].is_array());
    assert!(v["unmanaged"].is_array());
}

#[test]
fn manage_list_json_matches_status_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "manage", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["managed"].is_array());
    assert!(v["unmanaged"].is_array());
}

#[test]
fn sync_json_is_single_document() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "sync", "--fast"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    for key in [
        "managed",
        "discovered",
        "links_reconciled",
        "links_created",
        "projects_resynced",
        "files_updated",
        "integrity_issues",
    ] {
        assert!(v.get(key).is_some(), "sync report missing key {key}");
    }
}

#[test]
fn sync_adopt_json_is_single_document() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join(".claude").join("skills").join("demo-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo skill for tests\n---\n\n# Demo\n",
    )
    .unwrap();

    let out = agentspec(tmp.path())
        .args(["--format", "json", "sync", "--fast", "--adopt"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["managed"], 1);
    assert!(
        tmp.path()
            .join(".agents")
            .join("skills")
            .join("demo-skill")
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn mcp_list_json_includes_store() {
    let tmp = tempfile::tempdir().unwrap();
    let mcp_dir = tmp.path().join(".agents").join("mcp");
    fs::create_dir_all(&mcp_dir).unwrap();
    fs::write(
        mcp_dir.join("test.json"),
        r#"{"command":"echo","args":["hi"]}"#,
    )
    .unwrap();

    let out = agentspec(tmp.path())
        .args(["--format", "json", "mcp", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["store"]["test"]["command"], "echo");
    assert!(v["tools"].is_object());
}

#[test]
fn mcp_sync_json_reports_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "mcp", "sync"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["synced"].is_number());
    assert!(v["tools"].is_number());
}

#[test]
fn session_list_json_empty_is_empty_array() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".claude").join("projects")).unwrap();

    let out = agentspec(tmp.path())
        .args(["--format", "json", "session", "list", "claude"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v, serde_json::json!([]));
}

#[test]
fn session_list_and_export_json() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp
        .path()
        .join(".claude")
        .join("projects")
        .join("-tmp-proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("abc-123.jsonl"),
        concat!(
            r#"{"type":"user","cwd":"/tmp/proj","timestamp":"2026-01-02T03:04:05Z","message":{"content":"hello agentspec"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:04:06Z","message":{"content":[{"type":"text","text":"hi there"}]}}"#,
            "\n",
        ),
    )
    .unwrap();

    let out = agentspec(tmp.path())
        .args(["--format", "json", "session", "list", "claude"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    let sessions = v.as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "abc-123");
    assert_eq!(sessions[0]["first_prompt"], "hello agentspec");
    assert_eq!(sessions[0]["cwd"], "/tmp/proj");

    let out = agentspec(tmp.path())
        .args(["--format", "json", "session", "export", "claude", "--last"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["id"], "abc-123");
    assert_eq!(v["source"], "claude");
    assert!(v["markdown"].as_str().unwrap().contains("hello agentspec"));
}

#[test]
fn validate_json_reports_validity() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("demo-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo skill for tests\n---\n\n# Demo\n",
    )
    .unwrap();

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "manage",
            "validate",
            dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["valid"], true);
    assert_eq!(v["name"], "demo-skill");
    assert!(v["issues"].as_array().unwrap().is_empty());
}

#[test]
fn prune_json_defaults_to_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "prune"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["total"], 0);
}

#[test]
fn project_status_json_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "project", "status"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["synced"].is_array());
    assert!(v["desynced"].is_array());
    assert!(v["discovered"].is_array());
}

#[test]
fn verify_json_clean_store_exits_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "manage", "verify"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v, serde_json::json!([]));
}

#[test]
fn project_sync_same_basename_does_not_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let work = home.join("work").join("app");
    let personal = home.join("personal").join("app");
    for dir in [&work, &personal] {
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("AGENTS.md"), format!("# {}\n", dir.display())).unwrap();
    }

    for dir in [&work, &personal] {
        let out = agentspec(home)
            .args(["--format", "json", "project", "sync", dir.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success());
        let v = stdout_json(&out.stdout);
        assert_eq!(v["path"], dir.to_str().unwrap());
    }

    // Both projects must be tracked side by side, not overwrite each other.
    let out = agentspec(home)
        .args(["--format", "json", "project", "status"])
        .output()
        .unwrap();
    let v = stdout_json(&out.stdout);
    assert_eq!(v["synced"].as_array().unwrap().len(), 2);

    // Each store dir keeps its own AGENTS.md content.
    let store = home.join(".agents").join("projects");
    let mut contents: Vec<String> = fs::read_dir(&store)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| fs::read_to_string(e.path().join("AGENTS.md")).unwrap())
        .collect();
    contents.sort();
    assert_eq!(contents.len(), 2);
    assert_ne!(contents[0], contents[1]);

    // A bare ambiguous basename is rejected instead of guessing.
    let out = agentspec(home)
        .args(["project", "desync", "app"])
        .output()
        .unwrap();
    assert!(!out.status.success());

    // A full path still resolves.
    let out = agentspec(home)
        .args(["project", "desync", work.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn adopt_replaces_original_with_relative_symlink_then_prune_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let skill_dir = home.join(".claude").join("skills").join("demo-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo skill for tests\n---\n\n# Demo\n",
    )
    .unwrap();

    let out = agentspec(home)
        .args(["--format", "json", "sync", "--fast", "--adopt"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // The original tool copy is replaced by a relative symlink into the store.
    let link = home.join(".claude").join("skills").join("demo-skill");
    let meta = fs::symlink_metadata(&link).unwrap();
    assert!(meta.file_type().is_symlink(), "original must be a symlink");
    let target = fs::read_link(&link).unwrap();
    assert!(
        target.is_relative(),
        "symlink target must be relative: {target:?}"
    );
    assert!(
        fs::canonicalize(&link)
            .unwrap()
            .ends_with(".agents/skills/demo-skill")
    );
    assert!(
        home.join(".agents")
            .join("skills")
            .join("demo-skill")
            .join("SKILL.md")
            .exists()
    );

    // Deleting the store copy leaves a dead config entry plus a broken
    // symlink; prune --yes removes both.
    fs::remove_dir_all(home.join(".agents").join("skills").join("demo-skill")).unwrap();
    let out = agentspec(home)
        .args(["--format", "json", "prune", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["broken_resources"].as_array().unwrap().len(), 1);
    assert_eq!(v["broken_symlinks"].as_array().unwrap().len(), 1);
    assert!(
        fs::symlink_metadata(&link).is_err(),
        "broken symlink must be deleted"
    );

    // Store and config are consistent again.
    let out = agentspec(home)
        .args(["--format", "json", "manage", "verify"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(stdout_json(&out.stdout), serde_json::json!([]));

    let out = agentspec(home)
        .args(["--format", "json", "manage", "list"])
        .output()
        .unwrap();
    let v = stdout_json(&out.stdout);
    assert!(v["managed"].as_array().unwrap().is_empty());
}

#[test]
fn unlink_then_link_round_trips_the_tool_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let skill_dir = home.join(".claude").join("skills").join("linked-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: linked-skill\ndescription: Linked skill for tests\n---\n\n# Linked\n",
    )
    .unwrap();

    let out = agentspec(home)
        .args(["--format", "json", "sync", "--fast", "--adopt"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let link = home.join(".claude").join("skills").join("linked-skill");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let out = agentspec(home)
        .args(["manage", "unlink", "linked-skill", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::symlink_metadata(&link).is_err(),
        "unlink must remove the symlink"
    );

    let out = agentspec(home)
        .args(["manage", "link", "linked-skill", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        link.join("SKILL.md").exists(),
        "relinked symlink must resolve"
    );
}

#[test]
fn dedup_reports_identical_content_found_in_two_locations() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let content = "---\nname: twin\ndescription: Same content in two skills\n---\n\n# Twin\n";
    for name in ["twin-a", "twin-b"] {
        let dir = home.join(".claude").join("skills").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), content).unwrap();
    }

    // Populate the discovery cache.
    let out = agentspec(home)
        .args(["--format", "json", "status", "--fast"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = agentspec(home)
        .args(["--format", "json", "manage", "list", "--by-hash"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    let groups = v["by_hash"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "expected one duplicate group: {v}");
    assert_eq!(groups[0]["members"].as_array().unwrap().len(), 2);
}
