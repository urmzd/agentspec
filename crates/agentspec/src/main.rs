mod adapters;
mod cli;
mod config;
mod error;
mod frontmatter;
mod inventory;
mod ir;
mod jsonfile;
mod lockfile;
mod mcp;
mod ops;
mod project_files;
mod session;
mod tools;
mod tui;
mod update;

use clap::Parser;
use cli::{
    Cli, Command, FleetAction, FleetBackend, HooksAction, ManageAction, McpAction, OutputFormat,
    PermissionsAction, PlansAction, PluginsAction, ProjectAction, SessionAction,
    SessionContextMode, WorktreeAction,
};
use inventory::{Config, TrackedKind};
use ir::ResourceKind;

fn resolve_kind(cfg: &Config, name: &str) -> ResourceKind {
    let skill_dir = config::shared_skills_dir().join(name);
    if skill_dir.exists() {
        return ResourceKind::Skill;
    }
    let agent_file = config::shared_agents_dir().join(format!("{name}.md"));
    if agent_file.exists() {
        return ResourceKind::Agent;
    }
    // Fallback: check inventory config
    if let Some(r) = cfg.resources.iter().find(|r| r.name == name) {
        return r.kind.into();
    }
    // Default to skill if unknown
    ResourceKind::Skill
}

fn session_context_mode(context: SessionContextMode) -> session::route::ContextMode {
    match context {
        SessionContextMode::Brief => session::route::ContextMode::Brief,
        SessionContextMode::Full => session::route::ContextMode::Full,
    }
}

fn fleet_backend_selection(backend: FleetBackend) -> ops::fleet::BackendSelection {
    match backend {
        FleetBackend::Auto => ops::fleet::BackendSelection::Auto,
        FleetBackend::Store => ops::fleet::BackendSelection::Store,
        FleetBackend::Tmux => ops::fleet::BackendSelection::Tmux,
    }
}

fn active_backend_selection(
    requested: FleetBackend,
    active_backend: &str,
) -> error::Result<ops::fleet::BackendSelection> {
    if requested != FleetBackend::Auto {
        return Ok(fleet_backend_selection(requested));
    }

    match active_backend {
        "store" => Ok(ops::fleet::BackendSelection::Store),
        "tmux" => Ok(ops::fleet::BackendSelection::Tmux),
        other => Err(error::AppError::Other(format!(
            "unknown active fleet backend: {other}"
        ))),
    }
}

fn context_label(context: session::route::ContextMode) -> &'static str {
    match context {
        session::route::ContextMode::Brief => "brief",
        session::route::ContextMode::Full => "full",
    }
}

fn print_session_policy(json: bool) -> color_eyre::Result<()> {
    let policy = &session::route::ROUTING_POLICY;
    if json {
        println!("{}", serde_json::to_string_pretty(policy)?);
        return Ok(());
    }

    println!("Session Routing Policy");
    println!("Default context: {}", policy.default_context);
    println!();
    for mode in policy.modes {
        let marker = if mode.default { " (default)" } else { "" };
        println!("{}{}", mode.name, marker);
        println!(
            "  explicit: {}",
            if mode.requires_explicit_selection {
                "yes"
            } else {
                "no"
            }
        );
        println!("  includes: {}", mode.includes.join(", "));
        if mode.excludes.is_empty() {
            println!("  excludes: none");
        } else {
            println!("  excludes: {}", mode.excludes.join(", "));
        }
        println!("  limits: {}", mode.limits.join(", "));
    }
    println!();
    println!("Safeguards");
    for safeguard in policy.safeguards {
        println!("  - {safeguard}");
    }
    Ok(())
}

fn main() -> color_eyre::Result<()> {
    color_eyre::config::HookBuilder::default()
        .capture_span_trace_by_default(false)
        .display_env_section(false)
        .install()?;
    config::ensure_dirs()?;

    let cli = Cli::parse();

    match cli.command {
        None => {
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                tui::run()?;
            } else {
                let cfg = inventory::load_config()?;
                ops::list::list_skills(&cfg, None, true)?;
            }
        }
        Some(Command::Status { root, fast, path }) => {
            let mut cfg = inventory::load_config()?;
            let broad_root = if fast {
                None
            } else {
                Some(std::path::PathBuf::from(root.unwrap_or_else(|| {
                    dirs::home_dir().unwrap().to_string_lossy().to_string()
                })))
            };
            let extra_paths: Vec<std::path::PathBuf> = path
                .unwrap_or_default()
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect();
            ops::discover::refresh_cache_with_root(&mut cfg, broad_root.as_deref(), &extra_paths)?;
            ops::discover::status(&cfg, cli.format == OutputFormat::Json)?;
            inventory::save_config(&cfg)?;
        }
        Some(Command::Sync {
            root,
            fast,
            adopt,
            path,
        }) => {
            let mut cfg = inventory::load_config()?;
            let sync_root = root.map(std::path::PathBuf::from).or_else(dirs::home_dir);
            let extra_paths: Vec<std::path::PathBuf> = path
                .unwrap_or_default()
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect();
            ops::sync::sync(
                &mut cfg,
                sync_root.as_deref(),
                fast,
                adopt,
                cli.format == OutputFormat::Json,
                &extra_paths,
            )?;
            inventory::save_config(&cfg)?;
        }
        Some(Command::Project { action }) => {
            let mut cfg = inventory::load_config()?;
            match action {
                ProjectAction::Sync { project } => {
                    if let Some(name) = project {
                        ops::project_sync::sync_project(
                            &mut cfg,
                            &name,
                            cli.format == OutputFormat::Json,
                        )?;
                    } else {
                        ops::project_sync::sync_all(&mut cfg, cli.format == OutputFormat::Json)?;
                    }
                }
                ProjectAction::Desync { project } => {
                    ops::project_sync::desync_project(
                        &mut cfg,
                        &project,
                        cli.format == OutputFormat::Json,
                    )?;
                }
                ProjectAction::Remove { project } => {
                    ops::project_sync::remove_synced_project(
                        &mut cfg,
                        &project,
                        cli.format == OutputFormat::Json,
                    )?;
                }
                ProjectAction::Status { project } => {
                    ops::project_sync::project_status(
                        &cfg,
                        project.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
            }
            inventory::save_config(&cfg)?;
        }
        Some(Command::Session { action }) => match action {
            SessionAction::Policy => {
                print_session_policy(cli.format == OutputFormat::Json)?;
            }
            SessionAction::Find => {
                let (source, id) = session::find::run_find()?;
                println!("{source} {id}");
            }
            SessionAction::Active { pane } => {
                let active = session::active::active_sessions(pane.as_deref())?;
                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&active)?);
                } else if active.is_empty() {
                    eprintln!("No active fleet panes found");
                } else {
                    for item in active {
                        let session = item
                            .session
                            .as_ref()
                            .map(|s| format!("{}:{} ({})", s.source, s.id, s.reason))
                            .unwrap_or_else(|| "no session match".to_string());
                        println!(
                            "{}:{} | {} | {} | {}",
                            item.backend, item.pane, item.tool, item.state, session
                        );
                    }
                }
            }
            SessionAction::List { source } => {
                let adapter = session::get_adapter(&source)?;
                let sessions = adapter.list_sessions()?;
                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&sessions)?);
                } else if sessions.is_empty() {
                    eprintln!("No sessions found for {source}");
                } else {
                    for s in &sessions {
                        let date = s
                            .started_at
                            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let prompt = s.first_prompt.as_deref().unwrap_or("(no prompt)");
                        println!("{} | {} | {}", s.id, date, prompt);
                    }
                }
            }
            SessionAction::Export {
                source,
                id,
                last,
                output,
            } => {
                let adapter = session::get_adapter(&source)?;
                let sess = if last {
                    adapter.latest_session()?
                } else if let Some(id) = id {
                    adapter.load_session(&id)?
                } else {
                    return Err(error::AppError::Other(
                        "Provide a session ID or use --last".into(),
                    )
                    .into());
                };
                let markdown = session::render::render_markdown(&sess);
                let json = cli.format == OutputFormat::Json;
                if let Some(path) = output {
                    std::fs::write(&path, &markdown)?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "id": sess.id,
                                "source": source,
                                "path": path,
                            }))?
                        );
                    } else {
                        eprintln!("Written to {path}");
                    }
                } else if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "id": sess.id,
                            "source": source,
                            "markdown": markdown,
                        }))?
                    );
                } else {
                    print!("{markdown}");
                }
            }
            SessionAction::Sync {
                source,
                target,
                id,
                last,
                context,
                note,
            } => {
                let context = session_context_mode(context);
                let report = session::sync::sync_session(
                    &source,
                    &target,
                    id.as_deref(),
                    last,
                    context,
                    note.as_deref(),
                )?;
                if cli.format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "source": report.source,
                            "target": report.target,
                            "session_id": report.session_id,
                            "context": context_label(report.context),
                            "bytes": report.bytes,
                            "path": report.path.display().to_string(),
                        }))?
                    );
                }
            }
            SessionAction::Import { target, file } => {
                session::sync::import_session(&target, &file)?;
            }
            SessionAction::Route {
                source,
                pane,
                id,
                last,
                backend,
                context,
                note,
                dry_run,
            } => {
                let context = session_context_mode(context);
                if dry_run {
                    let preview = session::route::preview_route_context(
                        &source,
                        &pane,
                        id.as_deref(),
                        last,
                        context,
                        note.as_deref(),
                    )?;
                    if cli.format == OutputFormat::Json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "source": preview.source,
                                "session_id": preview.session_id,
                                "pane": preview.pane,
                                "context": context_label(preview.context),
                                "bytes": preview.bytes,
                                "dry_run": true,
                                "markdown": preview.markdown,
                            }))?
                        );
                    } else {
                        print!("{}", preview.markdown);
                    }
                    return Ok(());
                }

                let backend = fleet_backend_selection(backend);
                let report = session::route::route_session(
                    &source,
                    &pane,
                    id.as_deref(),
                    last,
                    backend,
                    context,
                    note.as_deref(),
                )?;
                if cli.format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "source": report.source,
                            "session_id": report.session_id,
                            "pane": report.pane,
                            "context": context_label(report.context),
                            "bytes": report.bytes,
                        }))?
                    );
                } else {
                    eprintln!(
                        "Routed {} context from {} session {} to {} ({} bytes)",
                        context_label(report.context),
                        report.source,
                        report.session_id,
                        report.pane,
                        report.bytes
                    );
                }
            }
            SessionAction::RouteActive {
                pane,
                backend,
                context,
                note,
                dry_run,
            } => {
                let active = session::active::best_for_pane(&pane)?;
                let active_backend = active.backend.clone();
                let Some(matched) = active.session else {
                    return Err(error::AppError::Other(format!(
                        "no matching session found for pane {pane}"
                    ))
                    .into());
                };
                let context = session_context_mode(context);
                if dry_run {
                    let preview = session::route::preview_route_context(
                        &matched.source,
                        &pane,
                        Some(&matched.id),
                        false,
                        context,
                        note.as_deref(),
                    )?;
                    if cli.format == OutputFormat::Json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "source": preview.source,
                                "session_id": preview.session_id,
                                "pane": preview.pane,
                                "context": context_label(preview.context),
                                "bytes": preview.bytes,
                                "dry_run": true,
                                "markdown": preview.markdown,
                                "matched": {
                                    "score": matched.score,
                                    "reason": matched.reason,
                                }
                            }))?
                        );
                    } else {
                        print!("{}", preview.markdown);
                    }
                    return Ok(());
                }

                let backend = active_backend_selection(backend, &active_backend)?;
                let report = session::route::route_session(
                    &matched.source,
                    &pane,
                    Some(&matched.id),
                    false,
                    backend,
                    context,
                    note.as_deref(),
                )?;
                if cli.format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "source": report.source,
                            "session_id": report.session_id,
                            "pane": report.pane,
                            "context": context_label(report.context),
                            "bytes": report.bytes,
                            "matched": {
                                "score": matched.score,
                                "reason": matched.reason,
                            }
                        }))?
                    );
                } else {
                    eprintln!(
                        "Routed matched {} session {} to {} ({} bytes)",
                        report.source, report.session_id, report.pane, report.bytes
                    );
                }
            }
            SessionAction::RouteFleet {
                fleet,
                backend,
                context,
                note,
                dry_run,
            } => {
                let context = session_context_mode(context);
                let active = session::active::active_sessions(None)?;
                let mut routed = Vec::new();
                let mut skipped = Vec::new();

                for item in active.into_iter().filter(|item| item.fleet == fleet) {
                    let Some(matched) = item.session else {
                        skipped.push(serde_json::json!({
                            "pane": item.pane,
                            "agent": item.agent,
                            "reason": "no-session-match",
                        }));
                        continue;
                    };

                    if dry_run {
                        let preview = session::route::preview_route_context(
                            &matched.source,
                            &item.pane,
                            Some(&matched.id),
                            false,
                            context,
                            note.as_deref(),
                        )?;
                        if cli.format != OutputFormat::Json {
                            println!(
                                "\n--- {} / {} -> {} ({}) ---\n",
                                matched.source, preview.session_id, item.pane, matched.reason
                            );
                            print!("{}", preview.markdown);
                        }
                        routed.push(serde_json::json!({
                            "pane": preview.pane,
                            "agent": item.agent,
                            "source": preview.source,
                            "session_id": preview.session_id,
                            "context": context_label(preview.context),
                            "bytes": preview.bytes,
                            "markdown": preview.markdown,
                            "matched": {
                                "score": matched.score,
                                "reason": matched.reason,
                            }
                        }));
                    } else {
                        let selected_backend = active_backend_selection(backend, &item.backend)?;
                        let report = session::route::route_session(
                            &matched.source,
                            &item.pane,
                            Some(&matched.id),
                            false,
                            selected_backend,
                            context,
                            note.as_deref(),
                        )?;
                        if cli.format != OutputFormat::Json {
                            eprintln!(
                                "Routed matched {} session {} to {} ({} bytes)",
                                report.source, report.session_id, report.pane, report.bytes
                            );
                        }
                        routed.push(serde_json::json!({
                            "pane": report.pane,
                            "agent": item.agent,
                            "source": report.source,
                            "session_id": report.session_id,
                            "context": context_label(report.context),
                            "bytes": report.bytes,
                            "matched": {
                                "score": matched.score,
                                "reason": matched.reason,
                            }
                        }));
                    }
                }

                if cli.format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "fleet": fleet,
                            "dry_run": dry_run,
                            "routed": routed,
                            "skipped": skipped,
                        }))?
                    );
                } else if routed.is_empty() {
                    eprintln!("No matched active sessions found for fleet {fleet}");
                }
            }
        },
        Some(Command::Prune { yes }) => {
            let mut cfg = inventory::load_config()?;
            ops::prune::prune(&mut cfg, !yes, cli.format == OutputFormat::Json)?;
            inventory::save_config(&cfg)?;
        }
        Some(Command::Update) => {
            eprintln!("current version: {}", env!("CARGO_PKG_VERSION"));
            match update::self_update("urmzd/agentspec", env!("CARGO_PKG_VERSION"), "agentspec")
                .map_err(|e| color_eyre::eyre::eyre!("{e:#}"))?
            {
                update::UpdateResult::AlreadyUpToDate => eprintln!("already up to date"),
                update::UpdateResult::Updated { from, to } => {
                    eprintln!("updated: {from} → {to}")
                }
            }
        }
        Some(Command::Version) => {
            println!("agentspec {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Command::Manage { action }) => {
            let mut cfg = inventory::load_config()?;
            let mut integrity_issues = 0;
            match action {
                ManageAction::Add {
                    source,
                    kind,
                    tools,
                    all_tools,
                    copy,
                } => {
                    let tracked_kind = kind.as_deref().map(|k| match k {
                        "skill" => TrackedKind::Skill,
                        "agent" => TrackedKind::Agent,
                        "project-config" => TrackedKind::ProjectConfig,
                        "instruction-file" => TrackedKind::InstructionFile,
                        "llms-txt" => TrackedKind::LlmsTxt,
                        "memory" => TrackedKind::Memory,
                        "session" => TrackedKind::Session,
                        "plan" => TrackedKind::Plan,
                        _ => unreachable!("clap validates this"),
                    });
                    let is_discovered = cfg
                        .discovered
                        .iter()
                        .any(|d| d.name == source && tracked_kind.is_none_or(|tk| d.kind == tk));
                    if is_discovered {
                        let kind = tracked_kind.unwrap_or_else(|| {
                            cfg.discovered
                                .iter()
                                .find(|d| d.name == source)
                                .unwrap()
                                .kind
                        });
                        ops::discover::adopt(
                            &mut cfg,
                            &source,
                            kind,
                            tools.as_deref(),
                            all_tools,
                            copy,
                        )?;
                    } else {
                        ops::manage::manage(&mut cfg, &source, tools.as_deref(), all_tools, copy)?;
                    }
                }
                ManageAction::Remove { name } => {
                    let kind = resolve_kind(&cfg, &name);
                    match kind {
                        ResourceKind::Skill => ops::remove::remove_skill(&mut cfg, &name)?,
                        ResourceKind::Agent => ops::remove::remove_agent(&mut cfg, &name)?,
                        _ => {
                            ops::remove::remove_tracked(&mut cfg, &name, kind.into())?;
                        }
                    }
                }
                ManageAction::All { all_tools, copy } => {
                    ops::discover::adopt_all(&mut cfg, all_tools, copy)?;
                }
                ManageAction::List {
                    dedup,
                    by_hash,
                    by_name,
                } => {
                    if dedup || by_hash || by_name {
                        ops::dedup::dedup(
                            &cfg,
                            by_hash,
                            by_name,
                            cli.format == OutputFormat::Json,
                        )?;
                    } else {
                        ops::discover::status(&cfg, cli.format == OutputFormat::Json)?;
                    }
                }
                ManageAction::Link { name, tool } => {
                    let kind = resolve_kind(&cfg, &name);
                    ops::link::link(&mut cfg, kind, &name, &tool, false)?;
                }
                ManageAction::Unlink { name, tool } => {
                    let kind = resolve_kind(&cfg, &name);
                    ops::link::unlink(&mut cfg, kind, &name, &tool)?;
                }
                ManageAction::Validate { path } => {
                    ops::validate::validate(path.as_deref(), cli.format == OutputFormat::Json)?;
                }
                ManageAction::Create { name, kind } => match kind.as_deref().unwrap_or("skill") {
                    "agent" => ops::create::create_agent(name.as_deref())?,
                    "project-config" => ops::create::create_project_config(name.as_deref())?,
                    "llms-txt" => ops::create::create_llms_txt(name.as_deref())?,
                    _ => ops::create::create_skill(name.as_deref())?,
                },
                ManageAction::Update { name } => {
                    if let Some(n) = name {
                        ops::refresh::update_resource(
                            &mut cfg,
                            &n,
                            cli.format == OutputFormat::Json,
                        )?;
                    } else {
                        ops::refresh::update_all(&mut cfg, cli.format == OutputFormat::Json)?;
                    }
                }
                ManageAction::Verify { accept, name } => {
                    integrity_issues = ops::verify::verify(
                        &mut cfg,
                        accept,
                        name.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                ManageAction::Memory {
                    project,
                    mem_type,
                    pull,
                    push,
                } => {
                    if pull {
                        ops::memory::sync_memories(
                            ops::memory::MemorySync::Pull,
                            project.as_deref(),
                            cli.format == OutputFormat::Json,
                        )?;
                    } else if push {
                        ops::memory::sync_memories(
                            ops::memory::MemorySync::Push,
                            project.as_deref(),
                            cli.format == OutputFormat::Json,
                        )?;
                    } else {
                        ops::memory::list_memories(
                            project.as_deref(),
                            mem_type.as_deref(),
                            cli.format == OutputFormat::Json,
                        )?;
                    }
                }
            }
            inventory::save_config(&cfg)?;
            // Exit only after the config is saved so verify never loses state.
            if integrity_issues > 0 {
                std::process::exit(1);
            }
        }
        Some(Command::Mcp { action }) => match action {
            McpAction::Add {
                name,
                command,
                args,
                env,
                url,
                server_type,
                tool,
            } => {
                let mut env_map = std::collections::HashMap::new();
                for pair in &env {
                    match pair.split_once('=') {
                        Some((k, v)) if !k.is_empty() => {
                            env_map.insert(k.to_string(), v.to_string());
                        }
                        _ => {
                            return Err(error::AppError::Other(format!(
                                "--env must be KEY=VALUE, got: {pair}"
                            ))
                            .into());
                        }
                    }
                }
                let server = mcp::McpServer {
                    command,
                    args,
                    env: env_map,
                    url,
                    server_type,
                };
                mcp::add_server(tool.as_deref(), &name, &server)?;
            }
            McpAction::Remove { name, tool, purge } => {
                mcp::remove_server(tool.as_deref(), &name, purge)?;
            }
            McpAction::List => {
                mcp::list_servers(cli.format == OutputFormat::Json)?;
            }
            McpAction::Link {
                name,
                tool,
                all_tools,
            } => {
                let target = if all_tools { None } else { tool.as_deref() };
                mcp::link_server(target, &name)?;
            }
            McpAction::Sync => {
                mcp::sync_all_servers(cli.format == OutputFormat::Json)?;
            }
        },
        Some(Command::Plans { action }) => {
            let mut cfg = inventory::load_config()?;
            match action {
                PlansAction::Import { source } => match source.as_str() {
                    "gemini" | "gemini-cli" | "gemini-antigravity" => {
                        ops::plans::import_gemini(&mut cfg, cli.format == OutputFormat::Json)?;
                    }
                    other => {
                        return Err(error::AppError::Other(format!(
                            "unknown plan source '{other}' (supported: gemini)"
                        ))
                        .into());
                    }
                },
                PlansAction::List => {
                    ops::plans::list_plans(cli.format == OutputFormat::Json)?;
                }
            }
            inventory::save_config(&cfg)?;
        }
        Some(Command::Permissions { action }) => match action {
            PermissionsAction::Init { force } => ops::permissions::init(force)?,
            PermissionsAction::Sync { tool, dry_run } => {
                ops::permissions::sync(tool.as_deref(), dry_run, cli.format == OutputFormat::Json)?
            }
            PermissionsAction::Show { tool } => {
                ops::permissions::show(tool.as_deref(), cli.format == OutputFormat::Json)?
            }
        },
        Some(Command::Plugins { action }) => match action {
            PluginsAction::List => ops::plugins::list_plugins(cli.format == OutputFormat::Json)?,
            PluginsAction::Export { output } => {
                ops::plugins::export_plugins(output.as_deref(), cli.format == OutputFormat::Json)?
            }
        },
        Some(Command::Hooks { action }) => match action {
            HooksAction::Add { path } => ops::hooks::add_hook(&path)?,
            HooksAction::List => ops::hooks::list_hooks(cli.format == OutputFormat::Json)?,
            HooksAction::Link {
                name,
                tool,
                all_tools,
            } => ops::hooks::link_hook(&name, tool.as_deref(), all_tools)?,
        },
        Some(Command::Fleet { backend, action }) => {
            let backend = match backend {
                FleetBackend::Auto => ops::fleet::BackendSelection::Auto,
                FleetBackend::Store => ops::fleet::BackendSelection::Store,
                FleetBackend::Tmux => ops::fleet::BackendSelection::Tmux,
            };
            match action {
                FleetAction::Doctor => {
                    ops::fleet::doctor(backend, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Survey { session } => {
                    ops::fleet::survey(
                        backend,
                        session.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Start { fleet } => {
                    ops::fleet::start(backend, &fleet, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Adopt {
                    fleet,
                    pane,
                    name,
                    tool,
                } => {
                    ops::fleet::adopt(
                        backend,
                        &fleet,
                        &pane,
                        name.as_deref(),
                        tool.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Group { fleet, name } => {
                    ops::fleet::group(backend, &fleet, &name, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Spawn {
                    fleet,
                    window,
                    tool,
                    name,
                    dir,
                    worktree,
                    repo,
                    branch,
                    base,
                } => {
                    if worktree.is_none() && (repo.is_some() || branch.is_some() || base.is_some())
                    {
                        return Err(error::AppError::Other(
                            "--repo, --branch, and --base require --worktree".into(),
                        )
                        .into());
                    }
                    let worktree_dir = match worktree {
                        Some(worktree) => {
                            let created = ops::worktree::ensure(
                                &worktree,
                                repo.as_deref(),
                                branch.as_deref(),
                                base.as_deref(),
                            )?;
                            Some(created.path.to_string_lossy().to_string())
                        }
                        None => None,
                    };
                    let dir = worktree_dir.as_deref().or(dir.as_deref());
                    ops::fleet::spawn(
                        backend,
                        &fleet,
                        &window,
                        &tool,
                        name.as_deref(),
                        dir,
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Send { pane, text } => {
                    ops::fleet::send(
                        backend,
                        &pane,
                        &text.join(" "),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Capture { pane, lines } => {
                    ops::fleet::capture(backend, &pane, lines, cli.format == OutputFormat::Json)?;
                }
                FleetAction::List { fleet } => {
                    ops::fleet::list(backend, &fleet, cli.format == OutputFormat::Json)?;
                }
                FleetAction::State { fleet, pane } => {
                    ops::fleet::state(backend, &fleet, &pane, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Mark {
                    fleet,
                    pane,
                    state,
                    note,
                } => {
                    ops::fleet::mark(
                        backend,
                        &fleet,
                        &pane,
                        &state,
                        note.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Event { fleet, line } => {
                    ops::fleet::event(
                        backend,
                        &fleet,
                        &line.join(" "),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Ping {
                    fleet,
                    message,
                    pane,
                } => {
                    ops::fleet::ping(
                        backend,
                        &fleet,
                        &message,
                        pane.as_deref(),
                        cli.format == OutputFormat::Json,
                    )?;
                }
                FleetAction::Dashboard { fleet } => {
                    ops::fleet::dashboard(backend, &fleet, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Attach { fleet } => {
                    ops::fleet::attach(backend, &fleet, cli.format == OutputFormat::Json)?;
                }
                FleetAction::Kill { fleet } => {
                    ops::fleet::kill(backend, &fleet, cli.format == OutputFormat::Json)?;
                }
            }
        }
        Some(Command::Worktree { action }) => match action {
            WorktreeAction::List { repo } => {
                ops::worktree::list(repo.as_deref(), cli.format == OutputFormat::Json)?;
            }
            WorktreeAction::Create {
                name,
                repo,
                branch,
                base,
            } => {
                ops::worktree::create(
                    &name,
                    repo.as_deref(),
                    branch.as_deref(),
                    base.as_deref(),
                    cli.format == OutputFormat::Json,
                )?;
            }
            WorktreeAction::Remove {
                target,
                repo,
                force,
                delete_branch,
            } => {
                ops::worktree::remove(
                    &target,
                    repo.as_deref(),
                    force,
                    delete_branch,
                    cli.format == OutputFormat::Json,
                )?;
            }
        },
    }

    Ok(())
}
