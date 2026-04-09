use crate::error::{AppError, Result};

pub struct Parsed {
    pub frontmatter: String,
    pub body: String,
}

pub fn parse(content: &str) -> Result<Parsed> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(AppError::InvalidFrontmatter(
            "file must start with ---".into(),
        ));
    }

    let after_first = &trimmed[3..];
    let end = after_first
        .find("\n---")
        .ok_or_else(|| AppError::InvalidFrontmatter("no closing --- found".into()))?;

    let frontmatter = after_first[..end].trim().to_string();
    let body = after_first[end + 4..].trim_start().to_string();

    Ok(Parsed { frontmatter, body })
}
