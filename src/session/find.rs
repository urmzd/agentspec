use crate::error::{AppError, Result};
use skim::prelude::*;
use std::io::Cursor;

use super::discover;
use super::ir::SessionMetaIR;

pub fn run_find() -> Result<(String, String)> {
    let all_sessions: Vec<SessionMetaIR> = discover::discover_all_sessions()?;

    if all_sessions.is_empty() {
        return Err(AppError::Other("No sessions found from any source".into()));
    }

    let lines: Vec<String> = all_sessions
        .iter()
        .map(|s| {
            let date = s
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let prompt = s.first_prompt.as_deref().unwrap_or("(no prompt)");
            let cwd = s
                .cwd
                .as_deref()
                .and_then(|c| c.rsplit('/').next())
                .unwrap_or("");
            format!("[{}] {} | {} | {}", s.tool_slug, date, cwd, prompt)
        })
        .collect();

    let input = lines.join("\n");

    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .prompt(Some("Select session> "))
        .build()
        .map_err(|e| AppError::Other(format!("Failed to build skim options: {e}")))?;

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    let output = Skim::run_with(&options, Some(items))
        .ok_or_else(|| AppError::Other("Skim was cancelled".into()))?;

    if output.is_abort {
        return Err(AppError::Other("Selection cancelled".into()));
    }

    let selected = output
        .selected_items
        .first()
        .ok_or_else(|| AppError::Other("No item selected".into()))?;

    let selected_text = selected.output().to_string();

    let idx = lines
        .iter()
        .position(|l| l == &selected_text)
        .ok_or_else(|| AppError::Other("Selected item not found".into()))?;

    let s = &all_sessions[idx];
    Ok((s.tool_slug.clone(), s.id.clone()))
}
