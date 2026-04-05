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
use cli::{
    AgentAction, Cli, Command, ManageAction, MemoryAction, SessionAction, SkillAction, ToolAction,
};
use inventory::TrackedKind;
use ir::ResourceKind;

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
        Some(Command::Skill { action }) => match action {
            SkillAction::List { tool } => {
                ops::list::list_skills(tool.as_deref(), cli.json)?;
            }
            SkillAction::Install {
                source,
                tools,
                all_tools,
            } => {
                ops::install::install_skill(&source, tools.as_deref(), all_tools)?;
            }
            SkillAction::Remove { name } => {
                ops::remove::remove_skill(&name)?;
            }
            SkillAction::Link { skill, tool } => {
                ops::link::link(ResourceKind::Skill, &skill, &tool, false)?;
            }
            SkillAction::Unlink { skill, tool } => {
                ops::link::unlink(ResourceKind::Skill, &skill, &tool)?;
            }
            SkillAction::Validate { path } => {
                ops::validate::validate(path.as_deref())?;
            }
            SkillAction::Create { name } => {
                ops::create::create_skill(name.as_deref())?;
            }
            SkillAction::Update { name: _ } => {
                println!("Update not yet implemented");
            }
        },
        Some(Command::Agent { action }) => match action {
            AgentAction::List { tool } => {
                ops::list::list_agents(tool.as_deref(), cli.json)?;
            }
            AgentAction::Install {
                source,
                tools,
                all_tools,
            } => {
                ops::install::install_agent(&source, tools.as_deref(), all_tools)?;
            }
            AgentAction::Remove { name } => {
                ops::remove::remove_agent(&name)?;
            }
            AgentAction::Link { agent, tool } => {
                ops::link::link(ResourceKind::Agent, &agent, &tool, false)?;
            }
            AgentAction::Unlink { agent, tool } => {
                ops::link::unlink(ResourceKind::Agent, &agent, &tool)?;
            }
            AgentAction::Validate { path } => {
                ops::validate::validate(path.as_deref())?;
            }
            AgentAction::Create { name } => {
                ops::create::create_agent(name.as_deref())?;
            }
        },
        Some(Command::Tool { action }) => match action {
            ToolAction::List => {
                ops::list::list_tools(cli.json)?;
            }
        },
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
        Some(Command::Search { query, limit }) => {
            ops::search::search(&query, limit, cli.json).await?;
        }
        Some(Command::Manage { action }) => match action {
            ManageAction::Add {
                source,
                kind,
                tools,
                all_tools,
                copy,
            } => {
                // Check if it's a discovered resource name
                let cfg = inventory::load_config()?;
                let tracked_kind = kind.as_deref().map(|k| match k {
                    "skill" => TrackedKind::Skill,
                    "agent" => TrackedKind::Agent,
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
            ManageAction::All { all_tools, copy } => {
                ops::discover::adopt_all(all_tools, copy)?;
            }
            ManageAction::List => {
                ops::discover::status(cli.json)?;
            }
        },
        Some(Command::Dedup { by_hash, by_name }) => {
            ops::dedup::dedup(by_hash, by_name, cli.json)?;
        }
        Some(Command::Memory { action }) => match action {
            MemoryAction::List { project, mem_type } => {
                ops::memory::list_memories(project.as_deref(), mem_type.as_deref(), cli.json)?;
            }
        },
        Some(Command::Discover) => {
            ops::discover::scan(cli.json)?;
        }
        Some(Command::Status) => {
            ops::discover::status(cli.json)?;
        }
        Some(Command::Verify { accept, name }) => {
            ops::verify::verify(accept, name.as_deref(), cli.json)?;
        }
    }

    Ok(())
}
