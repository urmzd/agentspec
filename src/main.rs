mod adapters;
#[allow(dead_code)]
mod agent;
mod cli;
mod config;
mod error;
mod frontmatter;
mod ir;
mod lockfile;
mod ops;
#[allow(dead_code)]
mod skill;
mod tools;
mod tui;

use clap::Parser;
use cli::{AgentAction, Cli, Command, SkillAction, ToolAction};
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
                ops::link::link(ResourceKind::Skill, &skill, &tool)?;
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
                ops::link::link(ResourceKind::Agent, &agent, &tool)?;
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
        Some(Command::Search { query, limit }) => {
            ops::search::search(&query, limit, cli.json).await?;
        }
    }

    Ok(())
}
