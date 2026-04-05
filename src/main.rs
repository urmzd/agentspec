mod adapters;
mod cli;
mod config;
mod error;
mod frontmatter;
mod inventory;
mod ir;
mod lockfile;
mod ops;
mod session;
mod tools;
mod tui;

use clap::Parser;
use cli::{Cli, Command, ManageAction, SessionAction};
use inventory::TrackedKind;
use ir::ResourceKind;

fn resolve_kind(name: &str) -> ResourceKind {
    let skill_dir = config::shared_skills_dir().join(name);
    if skill_dir.exists() {
        return ResourceKind::Skill;
    }
    let agent_file = config::shared_agents_dir().join(format!("{name}.md"));
    if agent_file.exists() {
        return ResourceKind::Agent;
    }
    // Fallback: check inventory config
    if let Ok(cfg) = inventory::load_config()
        && let Some(r) = cfg.resources.iter().find(|r| r.name == name)
    {
        return r.kind.into();
    }
    // Default to skill if unknown
    ResourceKind::Skill
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    config::ensure_dirs()?;

    let cli = Cli::parse();

    match cli.command {
        None => {
            if atty::is(atty::Stream::Stdout) {
                tui::run().await?;
            } else {
                ops::list::list_skills(None, true)?;
            }
        }
        Some(Command::Tui) => {
            tui::run().await?;
        }
        Some(Command::Status { root, fast }) => {
            let broad_root = if fast {
                None
            } else {
                Some(std::path::PathBuf::from(root.unwrap_or_else(|| {
                    dirs::home_dir().unwrap().to_string_lossy().to_string()
                })))
            };
            ops::discover::refresh_cache_with_root(broad_root.as_deref())?;
            ops::discover::status(cli.json)?;
        }
        Some(Command::Sync { root, fast, adopt }) => {
            let sync_root = root.map(std::path::PathBuf::from).or_else(dirs::home_dir);
            ops::sync::sync(sync_root.as_deref(), fast, adopt, cli.json)?;
        }
        Some(Command::Session { action }) => match action {
            SessionAction::Find => {
                let (source, id) = session::find::run_find()?;
                println!("{source} {id}");
            }
            SessionAction::List { source } => {
                let src = session::get_source(&source)?;
                let sessions = src.list_sessions()?;
                if sessions.is_empty() {
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
                let src = session::get_source(&source)?;
                let sess = if last {
                    src.latest_session()?
                } else if let Some(id) = id {
                    src.load_session(&id)?
                } else {
                    return Err(error::AppError::Other(
                        "Provide a session ID or use --last".into(),
                    )
                    .into());
                };
                let markdown = session::render::render_markdown(&sess);
                if let Some(path) = output {
                    std::fs::write(&path, &markdown)?;
                    eprintln!("Written to {path}");
                } else {
                    print!("{markdown}");
                }
            }
        },
        Some(Command::Manage { action }) => match action {
            ManageAction::Add {
                source,
                kind,
                tools,
                all_tools,
                copy,
            } => {
                let cfg = inventory::load_config()?;
                let tracked_kind = kind.as_deref().map(|k| match k {
                    "skill" => TrackedKind::Skill,
                    "agent" => TrackedKind::Agent,
                    "session" => TrackedKind::Session,
                    "memory" => TrackedKind::Memory,
                    "project-config" => TrackedKind::ProjectConfig,
                    "llms-txt" => TrackedKind::LlmsTxt,
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
                    ops::discover::adopt(&source, kind, tools.as_deref(), all_tools, copy)?;
                } else {
                    ops::manage::manage(&source, tools.as_deref(), all_tools, copy)?;
                }
            }
            ManageAction::Remove { name } => {
                let kind = resolve_kind(&name);
                match kind {
                    ResourceKind::Skill => ops::remove::remove_skill(&name)?,
                    ResourceKind::Agent => ops::remove::remove_agent(&name)?,
                    _ => {
                        ops::remove::remove_tracked(&name, kind.into())?;
                    }
                }
            }
            ManageAction::All { all_tools, copy } => {
                ops::discover::adopt_all(all_tools, copy)?;
            }
            ManageAction::List {
                dedup,
                by_hash,
                by_name,
            } => {
                if dedup || by_hash || by_name {
                    ops::dedup::dedup(by_hash, by_name, cli.json)?;
                } else {
                    ops::discover::status(cli.json)?;
                }
            }
            ManageAction::Link { name, tool } => {
                let kind = resolve_kind(&name);
                ops::link::link(kind, &name, &tool, false)?;
            }
            ManageAction::Unlink { name, tool } => {
                let kind = resolve_kind(&name);
                ops::link::unlink(kind, &name, &tool)?;
            }
            ManageAction::Validate { path } => {
                ops::validate::validate(path.as_deref())?;
            }
            ManageAction::Create { name, kind } => match kind.as_deref().unwrap_or("skill") {
                "agent" => ops::create::create_agent(name.as_deref())?,
                _ => ops::create::create_skill(name.as_deref())?,
            },
            ManageAction::Update { name: _ } => {
                println!("Update not yet implemented");
            }
            ManageAction::Verify { accept, name } => {
                ops::verify::verify(accept, name.as_deref(), cli.json)?;
            }
            ManageAction::Memory { project, mem_type } => {
                ops::memory::list_memories(project.as_deref(), mem_type.as_deref(), cli.json)?;
            }
        },
    }

    Ok(())
}
