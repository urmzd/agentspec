//! CLI integration tests pinning the `--format json` contract.
//!
//! Every json-emitting subcommand must write exactly one JSON document to
//! stdout — human progress lines belong on stderr or behind the human format.
//! Each test runs against a throwaway HOME so no real user state is touched.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command as StdCommand;

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

fn git(repo: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git command failed: {args:?}");
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
fn mcp_add_unlink_link_remove_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    // A ~/.claude dir makes claude-code count as installed (MCP config at
    // ~/.claude/settings.json).
    fs::create_dir_all(home.join(".claude").join("skills")).unwrap();
    let settings = home.join(".claude").join("settings.json");
    let store = home.join(".agents").join("mcp").join("srv.json");

    let out = agentspec(home)
        .args(["mcp", "add", "srv", "--command", "echo", "--args", "hi"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(store.exists(), "add must write the canonical store");
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["srv"]["command"], "echo");

    // Unlink removes the tool entry but keeps the store.
    let out = agentspec(home)
        .args(["mcp", "unlink", "srv", "--tool", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(v["mcpServers"].get("srv").is_none());
    assert!(store.exists(), "unlink must keep the canonical store");

    // Unlinking an unlinked server fails.
    let out = agentspec(home)
        .args(["mcp", "unlink", "srv"])
        .output()
        .unwrap();
    assert!(!out.status.success());

    // Link re-injects from the store.
    let out = agentspec(home)
        .args(["mcp", "link", "srv"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["srv"]["command"], "echo");

    // Remove deletes everywhere: tool configs and the store.
    let out = agentspec(home)
        .args(["mcp", "remove", "srv"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!store.exists(), "remove must delete the canonical store");
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(v["mcpServers"].get("srv").is_none());

    // Removing an unknown server fails.
    let out = agentspec(home)
        .args(["mcp", "remove", "srv"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn sync_adopts_project_mcp_json_into_store_and_links_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    fs::create_dir_all(home.join(".claude").join("skills")).unwrap();
    let mcp_json = r#"{"mcpServers":{"demo":{"command":"demo-server","args":["--stdio"]}}}"#;
    fs::write(home.join(".mcp.json"), mcp_json).unwrap();

    let out = agentspec(home)
        .args(["--format", "json", "sync", "--fast"])
        .output()
        .unwrap();
    assert!(out.status.success());
    stdout_json(&out.stdout);

    // The server is adopted into the canonical store, the original .mcp.json
    // is untouched, and the store copy is linked into tool configs.
    let store = home.join(".agents").join("mcp").join("demo.json");
    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&store).unwrap()).unwrap();
    assert_eq!(stored["command"], "demo-server");
    assert_eq!(
        fs::read_to_string(home.join(".mcp.json")).unwrap(),
        mcp_json
    );
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.join(".claude").join("settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["mcpServers"]["demo"]["command"], "demo-server");
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
fn session_sync_brief_context_stages_allowed_handoff_only() {
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
            r#"{"type":"system","timestamp":"2026-01-02T03:04:04Z","message":{"content":"hidden system prompt"}}"#,
            "\n",
            r#"{"type":"user","cwd":"/tmp/proj","timestamp":"2026-01-02T03:04:05Z","message":{"content":"continue the refactor"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:04:06Z","message":{"content":[{"type":"text","text":"I will inspect the files"},{"type":"tool_use","name":"Bash","input":{"command":"cat secret.txt"}}]}}"#,
            "\n",
        ),
    )
    .unwrap();

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "session",
            "sync",
            "claude",
            "codex",
            "--last",
            "--context",
            "brief",
            "--note",
            "Use this as background only.",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["source"], "claude-code");
    assert_eq!(v["target"], "codex");
    assert_eq!(v["session_id"], "abc-123");
    assert_eq!(v["context"], "brief");

    let path = v["path"].as_str().unwrap();
    let handoff = fs::read_to_string(path).unwrap();
    assert!(handoff.contains("Context policy: brief"));
    assert!(handoff.contains("Use this as background only."));
    assert!(handoff.contains("continue the refactor"));
    assert!(handoff.contains("I will inspect the files"));
    assert!(!handoff.contains("hidden system prompt"));
    assert!(!handoff.contains("cat secret.txt"));
}

#[test]
fn session_policy_json_describes_allowed_context() {
    let tmp = tempfile::tempdir().unwrap();
    let out = agentspec(tmp.path())
        .args(["--format", "json", "session", "policy"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);

    assert_eq!(v["default_context"], "brief");
    let modes = v["modes"].as_array().unwrap();
    let brief = modes.iter().find(|mode| mode["name"] == "brief").unwrap();
    assert_eq!(brief["default"], true);
    assert_eq!(brief["requires_explicit_selection"], false);
    assert!(brief["includes"].to_string().contains("user text"));
    assert!(brief["excludes"].to_string().contains("system prompts"));
    assert!(brief["excludes"].to_string().contains("tool results"));

    let full = modes.iter().find(|mode| mode["name"] == "full").unwrap();
    assert_eq!(full["requires_explicit_selection"], true);
    assert!(v["safeguards"].to_string().contains("--dry-run"));
}

#[test]
#[cfg(unix)]
fn fleet_survey_and_list_json_parse_helper_output() {
    let tmp = tempfile::tempdir().unwrap();
    let helper = tmp.path().join("fleet.sh");
    fs::write(
        &helper,
        r#"#!/bin/sh
case "$1" in
  survey)
    printf 'SESSION\tWINDOW\tPANE\tCOMMAND\tAGENT\tROLE\tNAME\tCWD\n'
    printf 'main\tapi\t%%7\tcodex\tyes\tagent\treviewer\t/tmp/repo\n'
    ;;
  list)
    printf 'WINDOW\tNAME\tTOOL\tSTATE\tPANE\n'
    printf 'api\treviewer\tcodex\tneeds-permission\t%%7\n'
    ;;
  doctor)
    printf 'tmux=ok\n'
    ;;
  start)
    printf 'FLEET=%s\nATTACH=tmux attach -t %s\n' "$2" "$2"
    ;;
  *)
    printf 'unknown command\n' >&2
    exit 2
    ;;
esac
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms).unwrap();

    let out = agentspec(tmp.path())
        .env("AGENTSPEC_FLEET_SH", &helper)
        .args(["--format", "json", "fleet", "--backend", "tmux", "survey"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["session"], "main");
    assert_eq!(v[0]["pane"], "%7");
    assert_eq!(v[0]["agent"], true);
    assert_eq!(v[0]["role"], "agent");

    let out = agentspec(tmp.path())
        .env("AGENTSPEC_FLEET_SH", &helper)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "tmux",
            "list",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["state"], "needs-permission");

    let out = agentspec(tmp.path())
        .env("AGENTSPEC_FLEET_SH", &helper)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "tmux",
            "start",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["FLEET"], "main");
}

#[test]
fn fleet_store_backend_works_without_tmux() {
    let tmp = tempfile::tempdir().unwrap();

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "start",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["backend"], "store");
    assert_eq!(v["FLEET"], "main");

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "spawn",
            "main",
            "api",
            "codex",
            "--name",
            "reviewer",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    let pane = v["PANE"].as_str().unwrap().to_string();
    assert!(pane.starts_with("store:main:reviewer"));

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "send",
            &pane,
            "review",
            "the",
            "diff",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["capture"].as_str().unwrap().contains("review the diff"));

    let guardian_line =
        format!(r#"GUARDIAN[{pane}]: needs-permission - "Approve edit?" - awaiting user decision"#);
    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "event",
            "main",
            &guardian_line,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["backend"], "store");
    assert_eq!(v["pane"], pane);
    assert_eq!(v["state"], "needs-permission");
    assert_eq!(v["summary"], r#""Approve edit?""#);

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "list",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["state"], "needs-permission");

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["capture"].as_str().unwrap().contains(&guardian_line));

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "mark",
            "main",
            &pane,
            "done",
            "--note",
            "Review completed.",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["state"], "done");

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "list",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["state"], "done");

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert!(v["capture"].as_str().unwrap().contains("Review completed."));

    let out = agentspec(tmp.path())
        .args(["--format", "json", "fleet", "--backend", "store", "survey"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["session"], "main");
    assert_eq!(v[0]["role"], "agent");

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "attach",
            "main",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["backend"], "store");
    assert_eq!(v["fleet"], "main");
    assert_eq!(v["command"], "agentspec fleet --backend store list main");
}

#[test]
fn fleet_spawn_can_create_managed_worktree_for_agent_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();

    let status = StdCommand::new("git")
        .arg("init")
        .arg(&repo)
        .status()
        .unwrap();
    assert!(status.success());
    git(&repo, &["config", "user.email", "agentspec@example.com"]);
    git(&repo, &["config", "user.name", "agentspec"]);
    fs::write(repo.join("README.md"), "# demo\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-m", "initial"]);

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "spawn",
            "work",
            "api",
            "codex",
            "--name",
            "api-agent",
            "--worktree",
            "api",
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(repo.join(".worktrees").join("api").exists());

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "survey",
            "work",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    let actual = fs::canonicalize(v[0]["cwd"].as_str().unwrap()).unwrap();
    let expected = fs::canonicalize(repo.join(".worktrees").join("api")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn session_route_brief_context_to_store_fleet_excludes_tool_output() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();

    let proj = home.join(".claude").join("projects").join("-tmp-proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("abc-123.jsonl"),
        concat!(
            r#"{"type":"system","timestamp":"2026-01-02T03:04:04Z","message":{"content":"hidden system prompt"}}"#,
            "\n",
            r#"{"type":"user","cwd":"/tmp/proj","timestamp":"2026-01-02T03:04:05Z","message":{"content":"continue the refactor"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:04:06Z","message":{"content":[{"type":"text","text":"I will inspect the files"},{"type":"tool_use","name":"Bash","input":{"command":"cat secret.txt"}}]}}"#,
            "\n",
        ),
    )
    .unwrap();

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "start",
            "handoff",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "spawn",
            "handoff",
            "api",
            "codex",
            "--name",
            "receiver",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let pane = stdout_json(&out.stdout)["PANE"]
        .as_str()
        .unwrap()
        .to_string();

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "session",
            "route",
            "claude",
            &pane,
            "--last",
            "--backend",
            "store",
            "--note",
            "Use this as background only.",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    let preview = v["markdown"].as_str().unwrap();
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["context"], "brief");
    assert!(preview.contains("continue the refactor"));
    assert!(preview.contains("I will inspect the files"));
    assert!(!preview.contains("hidden system prompt"));
    assert!(!preview.contains("cat secret.txt"));

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let capture = stdout_json(&out.stdout)["capture"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!capture.contains("continue the refactor"));

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "session",
            "route",
            "claude",
            &pane,
            "--last",
            "--backend",
            "store",
            "--note",
            "Use this as background only.",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["context"], "brief");
    assert_eq!(v["pane"], pane);

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let capture = stdout_json(&out.stdout)["capture"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(capture.contains("continue the refactor"));
    assert!(capture.contains("I will inspect the files"));
    assert!(capture.contains("Use this as background only."));
    assert!(!capture.contains("hidden system prompt"));
    assert!(!capture.contains("cat secret.txt"));
}

#[test]
fn session_active_and_route_active_match_store_pane_by_tool_and_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();

    let proj = home.join(".claude").join("projects").join("-tmp-proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("abc-123.jsonl"),
        concat!(
            r#"{"type":"user","cwd":"/tmp/proj","timestamp":"2026-01-02T03:04:05Z","message":{"content":"resume the active task"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:04:06Z","message":{"content":[{"type":"text","text":"active task noted"}]}}"#,
            "\n",
        ),
    )
    .unwrap();

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "spawn",
            "active",
            "api",
            "claude",
            "--name",
            "receiver",
            "--dir",
            "/tmp/proj",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let pane = stdout_json(&out.stdout)["PANE"]
        .as_str()
        .unwrap()
        .to_string();

    let out = agentspec(home)
        .args(["--format", "json", "session", "active", "--pane", &pane])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v[0]["pane"], pane);
    assert_eq!(v[0]["session"]["id"], "abc-123");
    assert_eq!(v[0]["session"]["reason"], "tool,cwd-exact");

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "session",
            "route-active",
            &pane,
            "--backend",
            "store",
            "--note",
            "Auto-routed context.",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["session_id"], "abc-123");
    assert_eq!(v["matched"]["reason"], "tool,cwd-exact");

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &pane,
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let capture = stdout_json(&out.stdout)["capture"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(capture.contains("resume the active task"));
    assert!(capture.contains("Auto-routed context."));
}

#[test]
fn session_route_fleet_routes_each_matched_active_pane() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();

    let proj_a = home.join(".claude").join("projects").join("-tmp-proj-a");
    let proj_b = home.join(".claude").join("projects").join("-tmp-proj-b");
    fs::create_dir_all(&proj_a).unwrap();
    fs::create_dir_all(&proj_b).unwrap();
    fs::write(
        proj_a.join("aaa-111.jsonl"),
        concat!(
            r#"{"type":"user","cwd":"/tmp/proj-a","timestamp":"2026-01-02T03:04:05Z","message":{"content":"continue API work"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:04:06Z","message":{"content":[{"type":"text","text":"API context ready"}]}}"#,
            "\n",
        ),
    )
    .unwrap();
    fs::write(
        proj_b.join("bbb-222.jsonl"),
        concat!(
            r#"{"type":"user","cwd":"/tmp/proj-b","timestamp":"2026-01-02T03:05:05Z","message":{"content":"continue UI work"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-02T03:05:06Z","message":{"content":[{"type":"text","text":"UI context ready"}]}}"#,
            "\n",
        ),
    )
    .unwrap();

    let mut panes = Vec::new();
    for (name, dir) in [
        ("api", "/tmp/proj-a"),
        ("ui", "/tmp/proj-b"),
        ("unmatched", "/tmp/no-session"),
    ] {
        let out = agentspec(home)
            .args([
                "--format",
                "json",
                "fleet",
                "--backend",
                "store",
                "spawn",
                "bulk",
                "work",
                "claude",
                "--name",
                name,
                "--dir",
                dir,
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        panes.push(
            stdout_json(&out.stdout)["PANE"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "session",
            "route-fleet",
            "bulk",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["routed"].as_array().unwrap().len(), 2);
    assert_eq!(v["skipped"].as_array().unwrap().len(), 1);
    assert!(v["routed"].to_string().contains("continue API work"));
    assert!(v["routed"].to_string().contains("continue UI work"));

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "session",
            "route-fleet",
            "bulk",
            "--note",
            "Bulk route context.",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["routed"].as_array().unwrap().len(), 2);
    assert_eq!(v["skipped"].as_array().unwrap().len(), 1);

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &panes[0],
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let capture = stdout_json(&out.stdout)["capture"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(capture.contains("continue API work"));
    assert!(capture.contains("Bulk route context."));

    let out = agentspec(home)
        .args([
            "--format",
            "json",
            "fleet",
            "--backend",
            "store",
            "capture",
            &panes[1],
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let capture = stdout_json(&out.stdout)["capture"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(capture.contains("continue UI work"));
    assert!(capture.contains("Bulk route context."));
}

#[test]
fn worktree_create_list_remove_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();

    let status = StdCommand::new("git")
        .arg("init")
        .arg(&repo)
        .status()
        .unwrap();
    assert!(status.success());
    git(&repo, &["config", "user.email", "agentspec@example.com"]);
    git(&repo, &["config", "user.name", "agentspec"]);
    fs::write(repo.join("README.md"), "# demo\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-m", "initial"]);

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "worktree",
            "create",
            "api",
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["branch"], "worktree-api");
    assert!(repo.join(".worktrees").join("api").exists());
    assert!(
        fs::read_to_string(repo.join(".git").join("info").join("exclude"))
            .unwrap()
            .contains(".worktrees/")
    );

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "worktree",
            "list",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    let entries = v.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .any(|entry| entry["branch"] == "worktree-api")
    );

    let out = agentspec(tmp.path())
        .args([
            "--format",
            "json",
            "worktree",
            "remove",
            "api",
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = stdout_json(&out.stdout);
    assert_eq!(v["branch"], "worktree-api");
    assert_eq!(v["branch_deleted"], true);
    assert!(!repo.join(".worktrees").join("api").exists());
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
fn adopt_copies_to_store_and_never_touches_the_original() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let skill_dir = home.join(".claude").join("skills").join("demo-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = "---\nname: demo-skill\ndescription: Demo skill for tests\n---\n\n# Demo\n";
    fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let out = agentspec(home)
        .args(["--format", "json", "sync", "--fast", "--adopt"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // The original stays a real directory with its content intact — adoption
    // copies to the store, it never leaves a symlink behind in the source.
    let original = home.join(".claude").join("skills").join("demo-skill");
    let meta = fs::symlink_metadata(&original).unwrap();
    assert!(
        meta.file_type().is_dir(),
        "original must remain a real directory"
    );
    assert_eq!(
        fs::read_to_string(original.join("SKILL.md")).unwrap(),
        skill_md
    );
    assert!(
        home.join(".agents")
            .join("skills")
            .join("demo-skill")
            .join("SKILL.md")
            .exists()
    );

    // The original is recorded as the resource's local source of truth.
    let out = agentspec(home)
        .args(["--format", "json", "manage", "list"])
        .output()
        .unwrap();
    let v = stdout_json(&out.stdout);
    let managed = v["managed"].as_array().unwrap();
    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0]["source_type"], "local");
    assert_eq!(managed[0]["source"], original.to_str().unwrap());

    // Deleting the store copy leaves a dead config entry; prune --yes removes
    // it without touching the original.
    fs::remove_dir_all(home.join(".agents").join("skills").join("demo-skill")).unwrap();
    let out = agentspec(home)
        .args(["--format", "json", "prune", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = stdout_json(&out.stdout);
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["broken_resources"].as_array().unwrap().len(), 1);
    assert!(
        original.join("SKILL.md").exists(),
        "prune must never delete the original"
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
fn unlink_then_link_round_trips_the_tool_copy() {
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

    // The tool-dir original is reconciled as a copy-strategy link.
    let link = home.join(".claude").join("skills").join("linked-skill");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_dir());

    let out = agentspec(home)
        .args(["manage", "unlink", "linked-skill", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::symlink_metadata(&link).is_err(),
        "unlink must remove the copied directory"
    );

    let out = agentspec(home)
        .args(["manage", "link", "linked-skill", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::symlink_metadata(&link).unwrap().file_type().is_dir(),
        "relink must create a real copy by default"
    );
    assert!(
        link.join("SKILL.md").exists(),
        "relinked copy must contain the skill"
    );

    // Symlink linking stays available as an explicit opt-in.
    let out = agentspec(home)
        .args(["manage", "unlink", "linked-skill", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = agentspec(home)
        .args(["manage", "link", "linked-skill", "claude-code", "--symlink"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "--symlink must create a symlink"
    );
    assert!(
        link.join("SKILL.md").exists(),
        "symlink must resolve into the store"
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
