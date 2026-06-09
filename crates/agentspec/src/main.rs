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
    Cli, Command, HooksAction, ManageAction, McpAction, OutputFormat, PermissionsAction,
    PlansAction, PluginsAction, ProjectAction, SessionAction,
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
            SessionAction::Find => {
                let (source, id) = session::find::run_find()?;
                println!("{source} {id}");
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
            } => {
                session::sync::sync_session(&source, &target, id.as_deref(), last)?;
            }
            SessionAction::Import { target, file } => {
                session::sync::import_session(&target, &file)?;
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
    }

    Ok(())
}
