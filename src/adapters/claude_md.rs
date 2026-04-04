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

        let mut r = Resource::new_simple(ResourceKind::ProjectConfig, name, description, body);
        r.source_path = Some(path.to_path_buf());
        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        Ok(format!("# {}\n\n{}", resource.name, resource.body))
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.body.is_empty() {
            issues.push("CLAUDE.md has no content".into());
        }
        issues
    }
}
