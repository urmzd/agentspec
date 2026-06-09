use std::path::Path;

use crate::error::Result;
use crate::ir::{Resource, ResourceKind};

use super::Adapter;

/// Adapter for CLAUDE.md project configuration files.
/// Plain markdown — name from first `#` heading, description from first paragraph.
pub struct ClaudeMdAdapter;

impl Adapter for ClaudeMdAdapter {
    fn vendor(&self) -> &str {
        "claude-md"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let (name, description, body) = super::agents_md::parse_heading_doc(&content);

        let r = Resource::new_simple(ResourceKind::ProjectConfig, name, description, body);
        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        Ok(super::agents_md::emit_heading_doc(
            &resource.name,
            &resource.description,
            &resource.body,
        ))
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.body.is_empty() {
            issues.push("CLAUDE.md has no content".into());
        }
        issues
    }
}
