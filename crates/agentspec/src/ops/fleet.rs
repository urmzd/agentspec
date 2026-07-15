//! Interface-first fleet management.
//!
//! `store` is a local persistent backend that works without tmux. `tmux` is a
//! native backend that delegates to the `orchestrate-agents` fleet helper and
//! preserves its tmux pane tagging contract.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};

#[derive(Clone, Copy, Debug)]
pub enum BackendSelection {
    Auto,
    Store,
    Tmux,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Backend {
    Store,
    Tmux,
}

#[derive(Debug)]
struct FleetOutput {
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SurveyPane {
    pub session: String,
    pub window: String,
    pub pane: String,
    pub command: String,
    pub agent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub cwd: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct FleetAgent {
    pub window: String,
    pub name: String,
    pub tool: String,
    pub state: String,
    pub pane: String,
}

#[derive(Debug, Clone)]
pub struct FleetStoreEntry {
    pub backend: String,
    pub fleet: String,
    pub window: String,
    pub name: String,
    pub tool: String,
    pub state: String,
    pub pane: String,
    pub cwd: Option<String>,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GuardianEvent {
    pub pane: String,
    pub state: String,
    pub summary: String,
    pub action: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GuardianEventReport {
    pub backend: String,
    pub fleet: String,
    pub pane: String,
    pub state: String,
    pub summary: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AttachCommand {
    pub backend: String,
    pub fleet: String,
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredFleet {
    name: String,
    backend: String,
    groups: Vec<String>,
    agents: Vec<StoredAgent>,
    events: Vec<StoredEvent>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredAgent {
    window: String,
    name: String,
    tool: String,
    state: String,
    pane: String,
    dir: Option<String>,
    messages: Vec<StoredMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredMessage {
    role: String,
    text: String,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredEvent {
    kind: String,
    pane: Option<String>,
    message: String,
    timestamp: String,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn resolve_backend(selection: BackendSelection) -> Backend {
    match selection {
        BackendSelection::Store => Backend::Store,
        BackendSelection::Tmux => Backend::Tmux,
        BackendSelection::Auto => {
            if tmux_usable() {
                Backend::Tmux
            } else {
                Backend::Store
            }
        }
    }
}

fn tmux_usable() -> bool {
    let Ok(helper) = helper_path() else {
        return false;
    };
    Command::new(helper)
        .arg("doctor")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn output_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_backend_note(backend: Backend) {
    match backend {
        Backend::Store => eprintln!("backend=store"),
        Backend::Tmux => eprintln!("backend=tmux"),
    }
}

// ---------------------------------------------------------------------------
// Public command functions
// ---------------------------------------------------------------------------

pub fn doctor(selection: BackendSelection, json: bool) -> Result<()> {
    let backend = resolve_backend(selection);
    let helper = helper_path().ok();
    let tmux = helper
        .as_ref()
        .and_then(|path| Command::new(path).arg("doctor").output().ok())
        .map(|out| {
            if out.status.success() {
                "ok".to_string()
            } else {
                "missing".to_string()
            }
        })
        .unwrap_or_else(|| "missing".to_string());

    let mut report = BTreeMap::new();
    report.insert("backend".to_string(), format!("{backend:?}").to_lowercase());
    report.insert("store".to_string(), "ok".to_string());
    report.insert("tmux".to_string(), tmux);
    if let Some(helper) = helper {
        report.insert("tmux_helper".to_string(), helper.display().to_string());
    }

    if json {
        output_json(&report)
    } else {
        for (key, value) in report {
            println!("{key}={value}");
        }
        Ok(())
    }
}

pub fn survey(selection: BackendSelection, session: Option<&str>, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_survey(session, json),
        Backend::Tmux => tmux_survey(session, json),
    }
}

pub fn start(selection: BackendSelection, fleet: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_start(fleet, json),
        Backend::Tmux => tmux_key_value(vec!["start".to_string(), fleet.to_string()], json),
    }
}

pub fn adopt(
    selection: BackendSelection,
    fleet: &str,
    pane: &str,
    name: Option<&str>,
    tool: Option<&str>,
    json: bool,
) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_adopt(fleet, pane, name, tool, json),
        Backend::Tmux => {
            let mut args = vec!["adopt".to_string(), fleet.to_string(), pane.to_string()];
            if let Some(name) = name {
                args.push("--name".to_string());
                args.push(name.to_string());
            }
            if let Some(tool) = tool {
                args.push("--tool".to_string());
                args.push(tool.to_string());
            }
            tmux_key_value(args, json)
        }
    }
}

pub fn group(selection: BackendSelection, fleet: &str, name: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_group(fleet, name, json),
        Backend::Tmux => tmux_key_value(
            vec!["group".to_string(), fleet.to_string(), name.to_string()],
            json,
        ),
    }
}

pub fn spawn(
    selection: BackendSelection,
    fleet: &str,
    window: &str,
    tool: &str,
    name: Option<&str>,
    dir: Option<&str>,
    json: bool,
) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_spawn(fleet, window, tool, name, dir, json),
        Backend::Tmux => {
            let mut args = vec![
                "spawn".to_string(),
                fleet.to_string(),
                window.to_string(),
                tool.to_string(),
            ];
            if let Some(name) = name {
                args.push("--name".to_string());
                args.push(name.to_string());
            }
            if let Some(dir) = dir {
                args.push("--dir".to_string());
                args.push(dir.to_string());
            }
            tmux_key_value(args, json)
        }
    }
}

pub fn spawn_silent(
    selection: BackendSelection,
    fleet: &str,
    window: &str,
    tool: &str,
    name: Option<&str>,
    dir: Option<&str>,
) -> Result<FleetAgent> {
    match resolve_backend(selection) {
        Backend::Store => store_spawn_silent(fleet, window, tool, name, dir),
        Backend::Tmux => tmux_spawn_silent(fleet, window, tool, name, dir),
    }
}

pub fn send(selection: BackendSelection, pane: &str, text: &str, json: bool) -> Result<()> {
    send_text(selection, pane, text)?;
    let backend = resolve_backend(selection);
    let value =
        serde_json::json!({ "backend": format!("{backend:?}").to_lowercase(), "SENT": pane });
    if json {
        output_json(&value)
    } else {
        println!("SENT={pane}");
        Ok(())
    }
}

pub fn send_text(selection: BackendSelection, pane: &str, text: &str) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_send_text(pane, text),
        Backend::Tmux => {
            tmux_run(&["send".to_string(), pane.to_string(), text.to_string()])?;
            Ok(())
        }
    }
}

pub fn capture(
    selection: BackendSelection,
    pane: &str,
    lines: Option<usize>,
    json: bool,
) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_capture(pane, lines, json),
        Backend::Tmux => {
            let mut args = vec!["capture".to_string(), pane.to_string()];
            if let Some(lines) = lines {
                args.push(lines.to_string());
            }
            let out = tmux_run(&args)?;
            let value =
                serde_json::json!({ "backend": "tmux", "pane": pane, "capture": out.stdout });
            tmux_print(out, json, value)
        }
    }
}

pub fn list(selection: BackendSelection, fleet: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_list(fleet, json),
        Backend::Tmux => tmux_list(fleet, json),
    }
}

pub fn state(selection: BackendSelection, fleet: &str, pane: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_state(fleet, pane, json),
        Backend::Tmux => {
            let state = tmux_effective_state(fleet, pane)?;
            let out = FleetOutput {
                stdout: format!("{state}\n"),
                stderr: String::new(),
            };
            let value = serde_json::json!({
                "backend": "tmux",
                "fleet": fleet,
                "pane": pane,
                "state": state,
            });
            tmux_print(out, json, value)
        }
    }
}

pub fn mark(
    selection: BackendSelection,
    fleet: &str,
    pane: &str,
    state: &str,
    note: Option<&str>,
    json: bool,
) -> Result<()> {
    validate_state(state)?;
    match resolve_backend(selection) {
        Backend::Store => store_mark(fleet, pane, state, note, json),
        Backend::Tmux => tmux_mark(fleet, pane, state, note, json),
    }
}

pub fn mark_silent(
    selection: BackendSelection,
    fleet: &str,
    pane: &str,
    state: &str,
    note: Option<&str>,
) -> Result<()> {
    validate_state(state)?;
    match resolve_backend(selection) {
        Backend::Store => store_mark_silent(fleet, pane, state, note),
        Backend::Tmux => tmux_mark_silent(fleet, pane, state, note),
    }
}

pub fn event(selection: BackendSelection, fleet: &str, line: &str, json: bool) -> Result<()> {
    let report = event_silent(selection, fleet, line)?;
    if json {
        output_json(&report)
    } else {
        println!("EVENT={}", report.pane);
        println!("STATE={}", report.state);
        Ok(())
    }
}

pub fn event_silent(
    selection: BackendSelection,
    fleet: &str,
    line: &str,
) -> Result<GuardianEventReport> {
    let event = parse_guardian_event(line)?;
    match resolve_backend(selection) {
        Backend::Store => store_guardian_event(fleet, &event),
        Backend::Tmux => tmux_guardian_event(fleet, &event),
    }
}

pub fn ping(
    selection: BackendSelection,
    fleet: &str,
    message: &str,
    pane: Option<&str>,
    json: bool,
) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_ping(fleet, message, pane, json),
        Backend::Tmux => {
            let mut args = vec!["ping".to_string(), fleet.to_string(), message.to_string()];
            if let Some(pane) = pane {
                args.push(pane.to_string());
            }
            tmux_key_value(args, json)
        }
    }
}

pub fn dashboard(selection: BackendSelection, fleet: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_dashboard(fleet, json),
        Backend::Tmux => tmux_key_value(vec!["dashboard".to_string(), fleet.to_string()], json),
    }
}

pub fn attach(selection: BackendSelection, fleet: &str, json: bool) -> Result<()> {
    let command = attach_command(selection, fleet)?;
    if json {
        output_json(&command)
    } else {
        println!("{}", command.command);
        Ok(())
    }
}

pub fn attach_command(selection: BackendSelection, fleet: &str) -> Result<AttachCommand> {
    match resolve_backend(selection) {
        Backend::Store => store_attach_command(fleet),
        Backend::Tmux => tmux_attach_command(fleet),
    }
}

pub fn kill(selection: BackendSelection, fleet: &str, json: bool) -> Result<()> {
    match resolve_backend(selection) {
        Backend::Store => store_kill(fleet, json),
        Backend::Tmux => tmux_key_value(vec!["kill".to_string(), fleet.to_string()], json),
    }
}

pub fn store_entries() -> Result<Vec<FleetStoreEntry>> {
    let mut entries: Vec<FleetStoreEntry> = all_stores()?
        .into_iter()
        .flat_map(|fleet| {
            let fleet_name = fleet.name;
            let updated_at = fleet.updated_at;
            fleet.agents.into_iter().map(move |agent| FleetStoreEntry {
                backend: "store".to_string(),
                fleet: fleet_name.clone(),
                window: agent.window,
                name: agent.name,
                tool: agent.tool,
                state: agent.state,
                pane: agent.pane,
                cwd: agent.dir,
                message_count: agent.messages.len(),
                last_message: agent
                    .messages
                    .last()
                    .map(|msg| first_line(&msg.text).to_string()),
                updated_at: updated_at.clone(),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        a.fleet
            .cmp(&b.fleet)
            .then_with(|| a.window.cmp(&b.window))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

pub fn active_entries() -> Result<Vec<FleetStoreEntry>> {
    let mut entries = store_entries()?;
    entries.extend(tmux_active_entries().unwrap_or_default());
    entries.sort_by(|a, b| {
        a.backend
            .cmp(&b.backend)
            .then_with(|| a.fleet.cmp(&b.fleet))
            .then_with(|| a.window.cmp(&b.window))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

pub fn active_count() -> usize {
    active_entries().map(|entries| entries.len()).unwrap_or(0)
}

pub fn render_pane_markdown(backend: &str, pane: &str) -> Result<String> {
    match backend {
        "store" => render_store_pane_markdown(pane),
        "tmux" => render_tmux_pane_markdown(pane),
        other => Err(AppError::Other(format!("unknown fleet backend: {other}"))),
    }
}

pub fn render_store_pane_markdown(pane: &str) -> Result<String> {
    for fleet in all_stores()? {
        if let Some(agent) = fleet.agents.iter().find(|agent| agent.pane == pane) {
            return Ok(render_store_agent_markdown(&fleet, agent));
        }
    }
    Err(AppError::Other(format!(
        "pane not found in store backend: {pane}"
    )))
}

fn render_store_agent_markdown(fleet: &StoredFleet, agent: &StoredAgent) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Fleet Agent: {}\n\n", agent.name));
    out.push_str(&format!("- Fleet: {}\n", fleet.name));
    out.push_str(&format!("- Window: {}\n", agent.window));
    out.push_str(&format!("- Tool: {}\n", agent.tool));
    out.push_str(&format!("- State: {}\n", agent.state));
    out.push_str(&format!("- Pane: {}\n", agent.pane));
    if let Some(dir) = &agent.dir {
        out.push_str(&format!("- Directory: {dir}\n"));
    }

    out.push_str("\n## Events\n\n");
    let relevant_events: Vec<&StoredEvent> = fleet
        .events
        .iter()
        .filter(|event| match event.pane.as_deref() {
            Some(pane) => pane == agent.pane,
            None => true,
        })
        .collect();
    if relevant_events.is_empty() {
        out.push_str("(no events)\n");
    } else {
        for event in relevant_events {
            let target = event
                .pane
                .as_deref()
                .map(|pane| format!(" pane {pane}"))
                .unwrap_or_else(|| " fleet".to_string());
            out.push_str(&format!(
                "### {}{} ({})\n\n",
                event.kind, target, event.timestamp
            ));
            out.push_str(&event.message);
            if !event.message.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
    }

    out.push_str("\n## Messages\n\n");
    if agent.messages.is_empty() {
        out.push_str("(no messages)\n");
    } else {
        for msg in &agent.messages {
            out.push_str(&format!("### {} ({})\n\n", msg.role, msg.timestamp));
            out.push_str(&msg.text);
            if !msg.text.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
    }
    out
}

fn tmux_active_entries() -> Result<Vec<FleetStoreEntry>> {
    if !tmux_usable() {
        return Ok(vec![]);
    }
    let out = tmux_run_literal(&["survey"])?;
    Ok(entries_from_tmux_survey(&out.stdout))
}

fn entries_from_tmux_survey(s: &str) -> Vec<FleetStoreEntry> {
    parse_survey(s)
        .into_iter()
        .filter(|pane| pane.agent || pane.role.as_deref() == Some("agent"))
        .map(|pane| {
            let name = pane.name.clone().unwrap_or_else(|| pane.command.clone());
            let manual_state = tmux_pane_option(&pane.pane, "@fleet_state").ok().flatten();
            let note = tmux_pane_option(&pane.pane, "@fleet_note").ok().flatten();
            FleetStoreEntry {
                backend: "tmux".to_string(),
                fleet: pane.session,
                window: pane.window,
                name,
                tool: pane.command,
                state: manual_state.unwrap_or_else(|| {
                    if pane.role.as_deref() == Some("agent") {
                        "tagged".to_string()
                    } else {
                        "detected".to_string()
                    }
                }),
                pane: pane.pane,
                cwd: Some(pane.cwd.clone()),
                message_count: 0,
                last_message: note.or(Some(pane.cwd)),
                updated_at: "live".to_string(),
            }
        })
        .collect()
}

fn render_tmux_pane_markdown(pane: &str) -> Result<String> {
    let out = tmux_run(&["capture".to_string(), pane.to_string(), "120".to_string()])?;
    let mut md = String::new();
    md.push_str("# Fleet Pane Capture\n\n");
    md.push_str(&format!("- Backend: tmux\n- Pane: {pane}\n\n"));
    md.push_str("## Recent Output\n\n```text\n");
    md.push_str(out.stdout.trim_end());
    md.push_str("\n```\n");
    Ok(md)
}

// ---------------------------------------------------------------------------
// Store backend
// ---------------------------------------------------------------------------

fn fleet_file(name: &str) -> PathBuf {
    config::shared_fleets_dir().join(format!("{}.json", safe_stem(name)))
}

fn safe_stem(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = cleaned.trim_matches('.');
    if stem.is_empty() {
        "fleet".to_string()
    } else {
        stem.to_string()
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

fn validate_state(state: &str) -> Result<()> {
    match state {
        "running" | "idle" | "needs-permission" | "error" | "stuck" | "done" | "relayed" => Ok(()),
        _ => Err(AppError::Other(format!("unsupported fleet state: {state}"))),
    }
}

pub fn parse_guardian_event(line: &str) -> Result<GuardianEvent> {
    let line = line.trim();
    let Some(rest) = line.strip_prefix("GUARDIAN[") else {
        return Err(AppError::Other(
            "guardian event must start with GUARDIAN[<pane>]: ".into(),
        ));
    };
    let Some((pane, payload)) = rest.split_once("]: ") else {
        return Err(AppError::Other(
            "guardian event must include ]: after pane id".into(),
        ));
    };
    if pane.trim().is_empty() {
        return Err(AppError::Other("guardian event pane is empty".into()));
    }

    let mut parts = payload.splitn(3, " - ");
    let state = parts.next().unwrap_or_default().trim();
    let summary = parts.next().unwrap_or_default().trim();
    let action = parts.next().unwrap_or_default().trim();
    if state.is_empty() || summary.is_empty() || action.is_empty() {
        return Err(AppError::Other(
            "guardian event must be: GUARDIAN[<pane>]: <state> - <summary> - <action>".into(),
        ));
    }
    validate_state(state)?;

    Ok(GuardianEvent {
        pane: pane.to_string(),
        state: state.to_string(),
        summary: summary.to_string(),
        action: action.to_string(),
        line: line.to_string(),
    })
}

fn load_store(name: &str) -> Result<StoredFleet> {
    let path = fleet_file(name);
    if !path.is_file() {
        return Err(AppError::Other(format!("fleet not found: {name}")));
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn save_store(fleet: &mut StoredFleet) -> Result<()> {
    std::fs::create_dir_all(config::shared_fleets_dir())?;
    fleet.updated_at = now();
    std::fs::write(
        fleet_file(&fleet.name),
        serde_json::to_string_pretty(fleet)?,
    )?;
    Ok(())
}

fn new_store(name: &str) -> StoredFleet {
    let timestamp = now();
    StoredFleet {
        name: name.to_string(),
        backend: "store".to_string(),
        groups: vec!["control".to_string()],
        agents: vec![],
        events: vec![],
        created_at: timestamp.clone(),
        updated_at: timestamp,
    }
}

fn all_stores() -> Result<Vec<StoredFleet>> {
    let dir = config::shared_fleets_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut fleets: Vec<StoredFleet> = Vec::new();
    for entry in std::fs::read_dir(dir)?.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            fleets.push(serde_json::from_str(&std::fs::read_to_string(path)?)?);
        }
    }
    fleets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fleets)
}

fn store_start(name: &str, json: bool) -> Result<()> {
    let existing = fleet_file(name).is_file();
    let mut fleet = load_store(name).unwrap_or_else(|_| new_store(name));
    save_store(&mut fleet)?;
    let value = serde_json::json!({
        "backend": "store",
        "FLEET": fleet.name,
        "EXISTING": existing,
    });
    if json {
        output_json(&value)
    } else {
        println!("FLEET={}", fleet.name);
        print_backend_note(Backend::Store);
        Ok(())
    }
}

fn store_survey(session: Option<&str>, json: bool) -> Result<()> {
    let stores = if let Some(session) = session {
        vec![load_store(session)?]
    } else {
        all_stores()?
    };
    let rows: Vec<SurveyPane> = stores
        .into_iter()
        .flat_map(|fleet| {
            fleet.agents.into_iter().map(move |agent| SurveyPane {
                session: fleet.name.clone(),
                window: agent.window,
                pane: agent.pane,
                command: agent.tool,
                agent: true,
                role: Some("agent".to_string()),
                name: Some(agent.name),
                cwd: agent.dir.unwrap_or_default(),
            })
        })
        .collect();
    if json {
        output_json(&rows)
    } else {
        println!("SESSION\tWINDOW\tPANE\tCOMMAND\tAGENT\tROLE\tNAME\tCWD");
        for row in rows {
            println!(
                "{}\t{}\t{}\t{}\tyes\t{}\t{}\t{}",
                row.session,
                row.window,
                row.pane,
                row.command,
                row.role.unwrap_or_else(|| "-".to_string()),
                row.name.unwrap_or_else(|| "-".to_string()),
                row.cwd
            );
        }
        Ok(())
    }
}

fn store_adopt(
    fleet: &str,
    pane: &str,
    name: Option<&str>,
    tool: Option<&str>,
    json: bool,
) -> Result<()> {
    let mut store = load_store(fleet).unwrap_or_else(|_| new_store(fleet));
    let tool = tool.unwrap_or("agent").to_string();
    let name = name.unwrap_or(&tool).to_string();
    upsert_agent(
        &mut store,
        StoredAgent {
            window: "adopted".to_string(),
            name: name.clone(),
            tool: tool.clone(),
            state: "running".to_string(),
            pane: pane.to_string(),
            dir: None,
            messages: vec![],
        },
    );
    save_store(&mut store)?;
    let value = serde_json::json!({ "backend": "store", "PANE": pane, "TOOL": tool, "NAME": name });
    if json {
        output_json(&value)
    } else {
        println!("PANE={pane}");
        println!("TOOL={tool}");
        println!("NAME={name}");
        Ok(())
    }
}

fn store_group(fleet: &str, name: &str, json: bool) -> Result<()> {
    let mut store = load_store(fleet).unwrap_or_else(|_| new_store(fleet));
    if !store.groups.iter().any(|group| group == name) {
        store.groups.push(name.to_string());
    }
    save_store(&mut store)?;
    let value = serde_json::json!({ "backend": "store", "WINDOW": name });
    if json {
        output_json(&value)
    } else {
        println!("WINDOW={name}");
        Ok(())
    }
}

fn store_spawn(
    fleet: &str,
    window: &str,
    tool: &str,
    name: Option<&str>,
    dir: Option<&str>,
    json: bool,
) -> Result<()> {
    let spawned = store_spawn_silent(fleet, window, tool, name, dir)?;
    let value = serde_json::json!({
        "backend": "store",
        "PANE": spawned.pane,
        "TOOL": spawned.tool,
        "NAME": spawned.name,
    });
    if json {
        output_json(&value)
    } else {
        println!("PANE={}", spawned.pane);
        println!("TOOL={}", spawned.tool);
        println!("NAME={}", spawned.name);
        Ok(())
    }
}

fn store_spawn_silent(
    fleet: &str,
    window: &str,
    tool: &str,
    name: Option<&str>,
    dir: Option<&str>,
) -> Result<FleetAgent> {
    let mut store = load_store(fleet).unwrap_or_else(|_| new_store(fleet));
    if !store.groups.iter().any(|group| group == window) {
        store.groups.push(window.to_string());
    }
    let name = name.unwrap_or(tool).to_string();
    let pane = format!(
        "store:{}:{}",
        safe_stem(fleet),
        safe_stem(&format!("{}-{}", name, store.agents.len() + 1))
    );
    upsert_agent(
        &mut store,
        StoredAgent {
            window: window.to_string(),
            name: name.clone(),
            tool: tool.to_string(),
            state: "idle".to_string(),
            pane: pane.clone(),
            dir: dir.map(str::to_string),
            messages: vec![],
        },
    );
    save_store(&mut store)?;
    Ok(FleetAgent {
        window: window.to_string(),
        name,
        tool: tool.to_string(),
        state: "idle".to_string(),
        pane,
    })
}

fn upsert_agent(store: &mut StoredFleet, agent: StoredAgent) {
    if let Some(existing) = store.agents.iter_mut().find(|a| a.pane == agent.pane) {
        *existing = agent;
    } else {
        store.agents.push(agent);
    }
}

fn with_agent_mut<T>(
    pane: &str,
    mut f: impl FnMut(&mut StoredFleet, usize) -> Result<T>,
) -> Result<T> {
    for mut fleet in all_stores()? {
        if let Some(idx) = fleet.agents.iter().position(|agent| agent.pane == pane) {
            let result = f(&mut fleet, idx)?;
            save_store(&mut fleet)?;
            return Ok(result);
        }
    }
    Err(AppError::Other(format!(
        "pane not found in store backend: {pane}"
    )))
}

fn store_send_text(pane: &str, text: &str) -> Result<()> {
    with_agent_mut(pane, |fleet, idx| {
        let agent = &mut fleet.agents[idx];
        agent.state = "running".to_string();
        agent.messages.push(StoredMessage {
            role: "user".to_string(),
            text: text.to_string(),
            timestamp: now(),
        });
        Ok(())
    })
}

fn store_capture(pane: &str, lines: Option<usize>, json: bool) -> Result<()> {
    let capture = all_stores()?
        .into_iter()
        .flat_map(|fleet| fleet.agents)
        .find(|agent| agent.pane == pane)
        .map(|agent| {
            let take = lines.unwrap_or(agent.messages.len());
            let start = agent.messages.len().saturating_sub(take);
            agent.messages[start..]
                .iter()
                .map(|msg| format!("{} [{}]: {}", msg.timestamp, msg.role, msg.text))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .ok_or_else(|| AppError::Other(format!("pane not found in store backend: {pane}")))?;

    if json {
        output_json(&serde_json::json!({ "backend": "store", "pane": pane, "capture": capture }))
    } else {
        println!("{capture}");
        Ok(())
    }
}

fn store_list(fleet: &str, json: bool) -> Result<()> {
    let store = load_store(fleet)?;
    let agents: Vec<FleetAgent> = store
        .agents
        .into_iter()
        .map(|agent| FleetAgent {
            window: agent.window,
            name: agent.name,
            tool: agent.tool,
            state: agent.state,
            pane: agent.pane,
        })
        .collect();
    if json {
        output_json(&agents)
    } else {
        println!("WINDOW\tNAME\tTOOL\tSTATE\tPANE");
        for agent in agents {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                agent.window, agent.name, agent.tool, agent.state, agent.pane
            );
        }
        Ok(())
    }
}

fn store_state(fleet: &str, pane: &str, json: bool) -> Result<()> {
    let store = load_store(fleet)?;
    let state = store
        .agents
        .iter()
        .find(|agent| agent.pane == pane)
        .map(|agent| agent.state.as_str())
        .ok_or_else(|| AppError::Other(format!("pane not found in fleet {fleet}: {pane}")))?;
    if json {
        output_json(&serde_json::json!({
            "backend": "store",
            "fleet": fleet,
            "pane": pane,
            "state": state,
        }))
    } else {
        println!("{state}");
        Ok(())
    }
}

fn store_mark(fleet: &str, pane: &str, state: &str, note: Option<&str>, json: bool) -> Result<()> {
    store_mark_silent(fleet, pane, state, note)?;
    let value = serde_json::json!({
        "backend": "store",
        "fleet": fleet,
        "pane": pane,
        "state": state,
        "note": note,
    });
    if json {
        output_json(&value)
    } else {
        println!("MARKED={pane}");
        println!("STATE={state}");
        Ok(())
    }
}

fn store_mark_silent(fleet: &str, pane: &str, state: &str, note: Option<&str>) -> Result<()> {
    let mut store = load_store(fleet)?;
    let Some(idx) = store.agents.iter().position(|agent| agent.pane == pane) else {
        return Err(AppError::Other(format!(
            "pane not found in fleet {fleet}: {pane}"
        )));
    };

    store.agents[idx].state = state.to_string();
    if let Some(note) = note.filter(|note| !note.trim().is_empty()) {
        store.agents[idx].messages.push(StoredMessage {
            role: "event".to_string(),
            text: note.trim().to_string(),
            timestamp: now(),
        });
        store.events.push(StoredEvent {
            kind: state.to_string(),
            pane: Some(pane.to_string()),
            message: note.trim().to_string(),
            timestamp: now(),
        });
    }
    save_store(&mut store)
}

fn store_guardian_event(fleet: &str, event: &GuardianEvent) -> Result<GuardianEventReport> {
    let mut store = load_store(fleet)?;
    let Some(idx) = store
        .agents
        .iter()
        .position(|agent| agent.pane == event.pane)
    else {
        return Err(AppError::Other(format!(
            "pane not found in fleet {fleet}: {}",
            event.pane
        )));
    };

    store.agents[idx].state = event.state.clone();
    store.agents[idx].messages.push(StoredMessage {
        role: "guardian".to_string(),
        text: event.line.clone(),
        timestamp: now(),
    });
    store.events.push(StoredEvent {
        kind: event.state.clone(),
        pane: Some(event.pane.clone()),
        message: event.line.clone(),
        timestamp: now(),
    });
    save_store(&mut store)?;

    Ok(GuardianEventReport {
        backend: "store".to_string(),
        fleet: fleet.to_string(),
        pane: event.pane.clone(),
        state: event.state.clone(),
        summary: event.summary.clone(),
        action: event.action.clone(),
    })
}

fn store_ping(fleet: &str, message: &str, pane: Option<&str>, json: bool) -> Result<()> {
    let mut store = load_store(fleet).unwrap_or_else(|_| new_store(fleet));
    store.events.push(StoredEvent {
        kind: "ping".to_string(),
        pane: pane.map(str::to_string),
        message: message.to_string(),
        timestamp: now(),
    });
    save_store(&mut store)?;
    let value = serde_json::json!({ "backend": "store", "PINGED": fleet, "message": message, "pane": pane });
    if json {
        output_json(&value)
    } else {
        eprintln!("PING[{fleet}]: {message}");
        println!("PINGED={fleet}");
        Ok(())
    }
}

fn store_dashboard(fleet: &str, json: bool) -> Result<()> {
    let store = load_store(fleet)?;
    let value = serde_json::json!({
        "backend": "store",
        "DASHBOARD": fleet,
        "agents": store.agents.len(),
        "events": store.events.len(),
    });
    if json {
        output_json(&value)
    } else {
        println!("DASHBOARD={fleet}");
        print_backend_note(Backend::Store);
        Ok(())
    }
}

fn store_attach_command(fleet: &str) -> Result<AttachCommand> {
    let _ = load_store(fleet)?;
    Ok(AttachCommand {
        backend: "store".to_string(),
        fleet: fleet.to_string(),
        command: format!("agentspec fleet --backend store list {fleet}"),
    })
}

fn tmux_attach_command(fleet: &str) -> Result<AttachCommand> {
    let out = tmux_run_literal(&["attach", fleet])?;
    Ok(AttachCommand {
        backend: "tmux".to_string(),
        fleet: fleet.to_string(),
        command: out.stdout.trim().to_string(),
    })
}

fn store_kill(fleet: &str, json: bool) -> Result<()> {
    let path = fleet_file(fleet);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if json {
        output_json(&serde_json::json!({ "backend": "store", "KILLED": fleet }))
    } else {
        println!("KILLED={fleet}");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tmux backend
// ---------------------------------------------------------------------------

fn helper_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("AGENTSPEC_FLEET_SH") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Ok(p);
        }
    }

    let home = dirs::home_dir().ok_or_else(|| AppError::Other("HOME is not set".into()))?;
    let candidates = [
        home.join(".agents/skills/orchestrate-agents/scripts/fleet.sh"),
        home.join(".agents/skills/orchestrate-agents/scripts/executable_fleet.sh"),
        home.join(".local/share/chezmoi/dot_agents/skills/orchestrate-agents/scripts/fleet.sh"),
        home.join(
            ".local/share/chezmoi/dot_agents/skills/orchestrate-agents/scripts/executable_fleet.sh",
        ),
    ];

    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        AppError::Other(
            "fleet helper not found; install/sync the orchestrate-agents skill or set AGENTSPEC_FLEET_SH"
                .into(),
        )
    })
}

fn tmux_run(args: &[String]) -> Result<FleetOutput> {
    let output = Command::new(helper_path()?).args(args).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let detail = stderr.trim();
        return Err(AppError::Other(if detail.is_empty() {
            format!("fleet helper failed with status {}", output.status)
        } else {
            detail.to_string()
        }));
    }
    Ok(FleetOutput { stdout, stderr })
}

fn tmux_run_literal(args: &[&str]) -> Result<FleetOutput> {
    let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    tmux_run(&owned)
}

fn tmux_set_pane_option(pane: &str, option: &str, value: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["set-option", "-p", "-t", pane, option, value])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Other(stderr.trim().to_string()));
    }
    Ok(())
}

fn tmux_pane_option(pane: &str, option: &str) -> Result<Option<String>> {
    let format = format!("#{{{option}}}");
    let output = Command::new("tmux")
        .args(["display-message", "-p", "-t", pane, &format])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn tmux_effective_state(fleet: &str, pane: &str) -> Result<String> {
    if let Some(state) = tmux_pane_option(pane, "@fleet_state")? {
        return Ok(state);
    }
    Ok(tmux_run_literal(&["state", fleet, pane])?
        .stdout
        .trim()
        .to_string())
}

fn tmux_mark(fleet: &str, pane: &str, state: &str, note: Option<&str>, json: bool) -> Result<()> {
    tmux_mark_silent(fleet, pane, state, note)?;
    let value = serde_json::json!({
        "backend": "tmux",
        "fleet": fleet,
        "pane": pane,
        "state": state,
        "note": note,
    });
    if json {
        output_json(&value)
    } else {
        println!("MARKED={pane}");
        println!("STATE={state}");
        Ok(())
    }
}

fn tmux_mark_silent(fleet: &str, pane: &str, state: &str, note: Option<&str>) -> Result<()> {
    let _ = tmux_run_literal(&["state", fleet, pane])?;
    tmux_set_pane_option(pane, "@fleet_state", state)?;
    tmux_set_pane_option(pane, "@fleet_note", note.unwrap_or(""))
}

fn tmux_print(out: FleetOutput, json: bool, value: serde_json::Value) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print!("{}", out.stdout);
        eprint!("{}", out.stderr);
    }
    Ok(())
}

fn tmux_guardian_event(fleet: &str, event: &GuardianEvent) -> Result<GuardianEventReport> {
    tmux_mark_silent(fleet, &event.pane, &event.state, Some(&event.line))?;
    Ok(GuardianEventReport {
        backend: "tmux".to_string(),
        fleet: fleet.to_string(),
        pane: event.pane.clone(),
        state: event.state.clone(),
        summary: event.summary.clone(),
        action: event.action.clone(),
    })
}

fn tmux_spawn_silent(
    fleet: &str,
    window: &str,
    tool: &str,
    name: Option<&str>,
    dir: Option<&str>,
) -> Result<FleetAgent> {
    let mut args = vec![
        "spawn".to_string(),
        fleet.to_string(),
        window.to_string(),
        tool.to_string(),
    ];
    if let Some(name) = name {
        args.push("--name".to_string());
        args.push(name.to_string());
    }
    if let Some(dir) = dir {
        args.push("--dir".to_string());
        args.push(dir.to_string());
    }
    let out = tmux_run(&args)?;
    let values = parse_key_values(&out.stdout);
    let pane = values
        .get("PANE")
        .cloned()
        .ok_or_else(|| AppError::Other("tmux spawn did not report PANE".to_string()))?;
    Ok(FleetAgent {
        window: window.to_string(),
        name: values
            .get("NAME")
            .cloned()
            .or_else(|| name.map(str::to_string))
            .unwrap_or_else(|| tool.to_string()),
        tool: values
            .get("TOOL")
            .cloned()
            .unwrap_or_else(|| tool.to_string()),
        state: "running".to_string(),
        pane,
    })
}

fn parse_key_values(s: &str) -> BTreeMap<String, String> {
    s.lines()
        .filter_map(|line| {
            line.split_once('=')
                .map(|(key, value)| (key.to_string(), value.to_string()))
        })
        .collect()
}

fn tmux_key_value(args: Vec<String>, json: bool) -> Result<()> {
    let out = tmux_run(&args)?;
    let mut values = parse_key_values(&out.stdout);
    values.insert("backend".to_string(), "tmux".to_string());
    tmux_print(out, json, serde_json::json!(values))
}

fn tmux_survey(session: Option<&str>, json: bool) -> Result<()> {
    let mut args = vec!["survey".to_string()];
    if let Some(session) = session {
        args.push(session.to_string());
    }
    let out = tmux_run(&args)?;
    let value = serde_json::json!(parse_survey(&out.stdout));
    tmux_print(out, json, value)
}

fn tmux_list(fleet: &str, json: bool) -> Result<()> {
    let out = tmux_run_literal(&["list", fleet])?;
    let mut agents = parse_agent_list(&out.stdout);
    for agent in &mut agents {
        if let Some(state) = tmux_pane_option(&agent.pane, "@fleet_state")? {
            agent.state = state;
        }
    }
    let stdout = render_agent_list_tsv(&agents);
    let value = serde_json::json!(agents);
    let out = FleetOutput {
        stdout,
        stderr: out.stderr,
    };
    tmux_print(out, json, value)
}

fn render_agent_list_tsv(agents: &[FleetAgent]) -> String {
    let mut out = String::from("WINDOW\tNAME\tTOOL\tSTATE\tPANE\n");
    for agent in agents {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            agent.window, agent.name, agent.tool, agent.state, agent.pane
        ));
    }
    out
}

pub fn parse_survey(s: &str) -> Vec<SurveyPane> {
    s.lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 8 {
                return None;
            }
            Some(SurveyPane {
                session: cols[0].to_string(),
                window: cols[1].to_string(),
                pane: cols[2].to_string(),
                command: cols[3].to_string(),
                agent: cols[4] == "yes",
                role: dash_none(cols[5]),
                name: dash_none(cols[6]),
                cwd: cols[7].to_string(),
            })
        })
        .collect()
}

pub fn parse_agent_list(s: &str) -> Vec<FleetAgent> {
    s.lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 5 {
                return None;
            }
            Some(FleetAgent {
                window: cols[0].to_string(),
                name: cols[1].to_string(),
                tool: cols[2].to_string(),
                state: cols[3].to_string(),
                pane: cols[4].to_string(),
            })
        })
        .collect()
}

fn dash_none(s: &str) -> Option<String> {
    if s == "-" || s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_survey_tsv() {
        let rows = parse_survey(
            "SESSION\tWINDOW\tPANE\tCOMMAND\tAGENT\tROLE\tNAME\tCWD\n\
             main\tapi\t%7\tcodex\tyes\tagent\treviewer\t/tmp/repo\n\
             main\tcontrol\t%1\tzsh\tno\t-\t-\t/tmp/repo\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pane, "%7");
        assert!(rows[0].agent);
        assert_eq!(rows[0].role.as_deref(), Some("agent"));
        assert_eq!(rows[1].role, None);
    }

    #[test]
    fn parses_fleet_list_tsv() {
        let rows = parse_agent_list(
            "WINDOW\tNAME\tTOOL\tSTATE\tPANE\n\
             api\treviewer\tcodex\tneeds-permission\t%7\n",
        );
        assert_eq!(
            rows,
            vec![FleetAgent {
                window: "api".into(),
                name: "reviewer".into(),
                tool: "codex".into(),
                state: "needs-permission".into(),
                pane: "%7".into(),
            }]
        );
    }

    #[test]
    fn parses_guardian_contract_line() {
        let event = parse_guardian_event(
            r#"GUARDIAN[%7]: needs-permission - "Approve edit?" - awaiting user decision"#,
        )
        .unwrap();

        assert_eq!(event.pane, "%7");
        assert_eq!(event.state, "needs-permission");
        assert_eq!(event.summary, r#""Approve edit?""#);
        assert_eq!(event.action, "awaiting user decision");
        assert!(parse_guardian_event("GUARDIAN[%7]: nope - x - y").is_err());
        assert!(parse_guardian_event("not a guardian line").is_err());
    }

    #[test]
    fn maps_tmux_survey_to_active_entries() {
        let entries = entries_from_tmux_survey(
            "SESSION\tWINDOW\tPANE\tCOMMAND\tAGENT\tROLE\tNAME\tCWD\n\
             fleet\tapi\t%7\tcodex\tyes\tagent\treviewer\t/tmp/repo\n\
             fleet\tcontrol\t%1\tzsh\tno\t-\t-\t/tmp/repo\n",
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].backend, "tmux");
        assert_eq!(entries[0].fleet, "fleet");
        assert_eq!(entries[0].pane, "%7");
        assert_eq!(entries[0].state, "tagged");
        assert_eq!(entries[0].last_message.as_deref(), Some("/tmp/repo"));
    }

    #[test]
    fn store_agent_markdown_includes_relevant_events() {
        let agent = StoredAgent {
            window: "api".into(),
            name: "reviewer".into(),
            tool: "codex".into(),
            state: "running".into(),
            pane: "store:main:reviewer-1".into(),
            dir: None,
            messages: vec![StoredMessage {
                role: "user".into(),
                text: "review this".into(),
                timestamp: "2026-01-02T03:04:05Z".into(),
            }],
        };
        let fleet = StoredFleet {
            name: "main".into(),
            backend: "store".into(),
            groups: vec!["api".into()],
            agents: vec![agent.clone()],
            events: vec![
                StoredEvent {
                    kind: "ping".into(),
                    pane: None,
                    message: "global ping".into(),
                    timestamp: "2026-01-02T03:04:06Z".into(),
                },
                StoredEvent {
                    kind: "needs-permission".into(),
                    pane: Some(agent.pane.clone()),
                    message: "pane event".into(),
                    timestamp: "2026-01-02T03:04:07Z".into(),
                },
                StoredEvent {
                    kind: "done".into(),
                    pane: Some("store:main:other-2".into()),
                    message: "other pane".into(),
                    timestamp: "2026-01-02T03:04:08Z".into(),
                },
            ],
            created_at: "2026-01-02T03:04:00Z".into(),
            updated_at: "2026-01-02T03:04:09Z".into(),
        };

        let rendered = render_store_agent_markdown(&fleet, &agent);
        assert!(rendered.contains("global ping"));
        assert!(rendered.contains("pane event"));
        assert!(rendered.contains("review this"));
        assert!(!rendered.contains("other pane"));
    }
}
