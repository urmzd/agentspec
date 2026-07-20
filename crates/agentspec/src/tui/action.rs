use std::path::PathBuf;

use crate::error::Result;
use crate::inventory::Config;
use crate::ir::ResourceKind;
use crate::mcp;
use crate::ops::{link as link_ops, remove as remove_ops};

// ---------------------------------------------------------------------------
// Agent provenance — where an agent lives on disk
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum AgentSource {
    /// Lives in ~/.agents/agents/ — the shared store.
    Managed,
    /// Lives only in tool-specific directories (not managed by agentspec).
    Unmanaged(Vec<PathBuf>),
}

// ---------------------------------------------------------------------------
// Actions — pure data describing a user intent
// ---------------------------------------------------------------------------

pub enum Action {
    DeleteSkill(String),
    DeleteAgent {
        name: String,
        source: AgentSource,
    },
    Link {
        kind: ResourceKind,
        name: String,
        tool: String,
    },
    Unlink {
        kind: ResourceKind,
        name: String,
        tool: String,
    },
    AddMcpServer {
        name: String,
        server: mcp::McpServer,
    },
    DeleteMcpServer(String),
    McpLink {
        name: String,
        tool: String,
    },
    McpUnlink {
        name: String,
        tool: String,
    },
}

pub enum ReloadTarget {
    SkillsAndAgents,
    Mcp,
}

impl Action {
    pub fn execute(&self, cfg: &mut Config) -> Result<()> {
        match self {
            Action::DeleteSkill(name) => remove_ops::remove_skill(cfg, name),
            Action::DeleteAgent { name, source } => match source {
                AgentSource::Managed => remove_ops::remove_agent(cfg, name),
                AgentSource::Unmanaged(paths) => remove_ops::remove_unmanaged_agent(name, paths),
            },
            Action::Link { kind, name, tool } => link_ops::link(cfg, *kind, name, tool, false),
            Action::Unlink { kind, name, tool } => link_ops::unlink(cfg, *kind, name, tool),
            Action::AddMcpServer { name, server } => mcp::add_server(None, name, server),
            Action::DeleteMcpServer(name) => mcp::remove_server(name),
            Action::McpLink { name, tool } => mcp::link_server(Some(tool), name),
            Action::McpUnlink { name, tool } => mcp::unlink_server(Some(tool), name),
        }
    }

    pub fn success_message(&self) -> String {
        match self {
            Action::DeleteSkill(name) => format!("Deleted skill '{name}'"),
            Action::DeleteAgent { name, .. } => format!("Deleted agent '{name}'"),
            Action::Link { name, tool, .. } => format!("Linked '{name}' to {tool}"),
            Action::Unlink { name, tool, .. } => format!("Unlinked '{name}' from {tool}"),
            Action::AddMcpServer { name, .. } => format!("Added MCP server '{name}'"),
            Action::DeleteMcpServer(name) => format!("Removed MCP server '{name}'"),
            Action::McpLink { name, tool } => format!("Linked '{name}' to {tool}"),
            Action::McpUnlink { name, tool } => format!("Unlinked '{name}' from {tool}"),
        }
    }

    pub fn reload_target(&self) -> ReloadTarget {
        match self {
            Action::AddMcpServer { .. }
            | Action::DeleteMcpServer(_)
            | Action::McpLink { .. }
            | Action::McpUnlink { .. } => ReloadTarget::Mcp,
            _ => ReloadTarget::SkillsAndAgents,
        }
    }
}
