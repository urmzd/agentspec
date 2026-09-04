//! Structured search across every AI coding session on the machine.
//!
//! `session list` answers "what sessions exist"; this answers "which session
//! was the one where I…". Sessions are filtered by metadata first (source,
//! project, date) because that is cheap, and only the survivors are opened and
//! scanned message by message. Role filters make the common case — "find my own
//! prompt, not the assistant's restatement of it" — a single flag.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use regex::RegexBuilder;
use serde::Serialize;

use crate::error::{AppError, Result};

use super::adapters;
use super::discover;
use super::ir::{ContentBlockIR, MessageIR, RoleIR, SessionIR, SessionMetaIR};

/// Everything the user asked for, resolved from CLI flags.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Text to look for. `None` lists sessions matching the metadata filters.
    pub text: Option<String>,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Tool slugs to search, empty means all available.
    pub sources: Vec<String>,
    /// Message roles a hit may come from, empty means all.
    pub roles: Vec<RoleIR>,
    /// Case-insensitive substring matched against project name and cwd.
    pub project: Option<String>,
    /// Case-insensitive substring matched against files touched.
    pub file: Option<String>,
    /// Exact tool name that must appear in the session's tool calls.
    pub tool_used: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    /// Max sessions returned.
    pub limit: usize,
    /// Max matches reported per session.
    pub hits_per_session: usize,
    /// Excerpt width in characters around a match.
    pub context: usize,
    /// Return whole matched messages instead of excerpts.
    pub full: bool,
    /// Ceiling on how many sessions are opened and scanned.
    pub scan: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            regex: false,
            case_sensitive: false,
            sources: Vec::new(),
            roles: Vec::new(),
            project: None,
            file: None,
            tool_used: None,
            since: None,
            until: None,
            limit: 20,
            hits_per_session: 3,
            context: 160,
            full: false,
            scan: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageMatch {
    /// Position of the message in the session transcript.
    pub index: usize,
    pub role: RoleIR,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    /// Which kind of content block matched: `text`, `tool_use`, `tool_result`.
    pub kind: &'static str,
    /// Tool name, when the hit came from a tool call or result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub tool: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    pub message_count: usize,
    /// How many messages matched in total, even when `matches` is truncated.
    pub match_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files_touched: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools_used: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<MessageMatch>,
    /// Command that prints this session in full.
    pub export_command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchReport {
    /// Sessions considered after metadata filtering.
    pub candidates: usize,
    /// Sessions actually opened and scanned.
    pub scanned: usize,
    /// Sessions with at least one hit.
    pub matched: usize,
    /// Whether the scan ceiling cut the candidate list short.
    pub truncated: bool,
    pub sources: Vec<String>,
    pub results: Vec<SearchResult>,
}

/// Parse `--role` values, including the `human` alias for `user`.
pub fn parse_role(s: &str) -> Result<RoleIR> {
    match s.to_ascii_lowercase().as_str() {
        "user" | "human" | "me" => Ok(RoleIR::User),
        "assistant" | "agent" | "ai" => Ok(RoleIR::Assistant),
        "system" => Ok(RoleIR::System),
        "tool" => Ok(RoleIR::Tool),
        other => Err(AppError::Other(format!(
            "unknown role '{other}' (use user, assistant, system, or tool)"
        ))),
    }
}

/// Parse an absolute date (`2026-01-31`, RFC 3339) or a relative age
/// (`30m`, `24h`, `7d`, `2w`) counted back from now.
pub fn parse_time(s: &str) -> Result<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(DateTime::from_naive_utc_and_offset(
            d.and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        ));
    }
    if let Some(delta) = parse_relative(s) {
        return Ok(Utc::now() - delta);
    }
    Err(AppError::Other(format!(
        "could not parse time '{s}' (use YYYY-MM-DD, RFC 3339, or a relative age like 7d)"
    )))
}

fn parse_relative(s: &str) -> Option<Duration> {
    let (num, unit) = s.split_at(s.find(|c: char| c.is_alphabetic())?);
    let n: i64 = num.parse().ok()?;
    match unit {
        "m" | "min" => Some(Duration::minutes(n)),
        "h" | "hr" => Some(Duration::hours(n)),
        "d" | "day" | "days" => Some(Duration::days(n)),
        "w" | "week" | "weeks" => Some(Duration::weeks(n)),
        _ => None,
    }
}

/// Compiled matcher for the query text. Absent text matches everything.
struct Matcher(Option<regex::Regex>);

impl Matcher {
    fn build(q: &SearchQuery) -> Result<Self> {
        let Some(text) = q.text.as_deref().filter(|t| !t.is_empty()) else {
            return Ok(Self(None));
        };
        let pattern = if q.regex {
            text.to_string()
        } else {
            regex::escape(text)
        };
        let re = RegexBuilder::new(&pattern)
            .case_insensitive(!q.case_sensitive)
            .build()
            .map_err(|e| AppError::Other(format!("invalid --regex pattern: {e}")))?;
        Ok(Self(Some(re)))
    }

    fn is_noop(&self) -> bool {
        self.0.is_none()
    }

    /// Byte offset of the first match, if any.
    fn find(&self, haystack: &str) -> Option<usize> {
        match &self.0 {
            Some(re) => re.find(haystack).map(|m| m.start()),
            None => Some(0),
        }
    }
}

fn contains_ci(haystack: Option<&str>, needle: &str) -> bool {
    haystack.is_some_and(|h| h.to_lowercase().contains(&needle.to_lowercase()))
}

/// Metadata-only filters, applied before any session is opened.
fn passes_meta(meta: &SessionMetaIR, q: &SearchQuery) -> bool {
    if !q.sources.is_empty() && !q.sources.iter().any(|s| slug_matches(s, &meta.tool_slug)) {
        return false;
    }
    if let Some(p) = &q.project
        && !(contains_ci(meta.project.as_deref(), p) || contains_ci(meta.cwd.as_deref(), p))
    {
        return false;
    }
    match (q.since, meta.started_at) {
        (Some(since), Some(started)) if started < since => return false,
        // A session with no timestamp cannot satisfy a date bound.
        (Some(_), None) => return false,
        _ => {}
    }
    match (q.until, meta.started_at) {
        (Some(until), Some(started)) if started > until => return false,
        (Some(_), None) => return false,
        _ => {}
    }
    true
}

/// Accept both the CLI alias and the canonical slug (`claude` / `claude-code`).
fn slug_matches(requested: &str, slug: &str) -> bool {
    requested == slug
        || adapters::adapter_for_tool(requested).is_some_and(|a| a.tool_slug() == slug)
}

/// Collapse runs of whitespace so an excerpt is one readable line. Matching
/// runs against this same collapsed form, so the match offset always lines up
/// with the window that gets cut.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cut a window of `width` characters centred on a match at byte offset `at`
/// within `collapsed`.
fn excerpt(collapsed: &str, at: usize, width: usize, full: bool) -> String {
    if full {
        return collapsed.to_string();
    }
    let chars: Vec<char> = collapsed.chars().collect();
    if chars.len() <= width {
        return collapsed.to_string();
    }
    let center = collapsed[..at.min(collapsed.len())].chars().count();
    let start = center.saturating_sub(width / 4);
    let end = (start + width).min(chars.len());
    let start = end.saturating_sub(width);
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

/// Searchable text for one content block, plus its kind and tool name.
fn block_text(block: &ContentBlockIR) -> (&'static str, Option<String>, String) {
    match block {
        ContentBlockIR::Text { text } => ("text", None, text.clone()),
        ContentBlockIR::ToolUse { name, input, .. } => {
            ("tool_use", Some(name.clone()), format!("{name} {input}"))
        }
        ContentBlockIR::ToolResult { content, .. } => ("tool_result", None, content.clone()),
        ContentBlockIR::Unknown { raw } => ("unknown", None, raw.to_string()),
    }
}

fn match_message(
    index: usize,
    msg: &MessageIR,
    matcher: &Matcher,
    q: &SearchQuery,
) -> Option<MessageMatch> {
    if !q.roles.is_empty() && !q.roles.contains(&msg.role) {
        return None;
    }
    for block in &msg.content {
        let (kind, tool, text) = block_text(block);
        let text = collapse(&text);
        if text.is_empty() {
            continue;
        }
        if let Some(at) = matcher.find(&text) {
            return Some(MessageMatch {
                index,
                role: msg.role,
                timestamp: msg.timestamp,
                kind,
                tool,
                excerpt: excerpt(&text, at, q.context, q.full),
            });
        }
    }
    None
}

fn export_command(tool_slug: &str, id: &str) -> String {
    let source = match tool_slug {
        "claude-code" => "claude",
        "github-copilot" => "copilot",
        "gemini-cli" => "gemini",
        other => other,
    };
    format!("agentspec session export {source} {id}")
}

fn to_result(session: &SessionIR, matches: Vec<MessageMatch>, match_count: usize) -> SearchResult {
    SearchResult {
        tool: session.tool_slug.clone(),
        id: session.id.clone(),
        project: session.project.clone(),
        cwd: session.cwd.clone(),
        branch: session.branch.clone(),
        started_at: session.started_at,
        summary: session.summary.clone(),
        first_prompt: session.first_prompt.clone(),
        message_count: session.messages.len(),
        match_count,
        files_touched: session.files_touched.clone(),
        tools_used: if session.tools_used.is_empty() {
            session.compute_tools_used()
        } else {
            session.tools_used.clone()
        },
        matches,
        export_command: export_command(&session.tool_slug, &session.id),
    }
}

/// Run the search. Sessions are visited newest first, so `--limit` returns the
/// most recent matches rather than an arbitrary slice.
pub fn search(q: &SearchQuery) -> Result<SearchReport> {
    let matcher = Matcher::build(q)?;
    let sources: Vec<String> = adapters::available_session_adapters()
        .iter()
        .map(|a| a.tool_slug().to_string())
        .filter(|slug| q.sources.is_empty() || q.sources.iter().any(|s| slug_matches(s, slug)))
        .collect();

    if !q.sources.is_empty() && sources.is_empty() {
        return Err(AppError::Other(format!(
            "no session storage found for: {}",
            q.sources.join(", ")
        )));
    }

    let candidates: Vec<SessionMetaIR> = discover::discover_all_sessions()?
        .into_iter()
        .filter(|m| passes_meta(m, q))
        .collect();

    // Roles narrow which messages a query may match. Without a query there is
    // nothing to narrow, and reporting every message of that role would bury
    // the result — say so instead of returning noise.
    if !q.roles.is_empty() && matcher.is_noop() {
        return Err(AppError::Other(
            "--role narrows which messages the query matches; give a query to search for".into(),
        ));
    }

    // Metadata-only filters can be answered without opening anything, but the
    // moment a text, file, or tool filter is present the transcript has to be
    // read.
    let needs_load = !matcher.is_noop() || q.file.is_some() || q.tool_used.is_some();

    let mut results = Vec::new();
    let mut scanned = 0usize;
    let mut truncated = false;

    for meta in &candidates {
        if results.len() >= q.limit {
            truncated = truncated || candidates.len() > scanned;
            break;
        }
        if !needs_load {
            results.push(SearchResult {
                tool: meta.tool_slug.clone(),
                id: meta.id.clone(),
                project: meta.project.clone(),
                cwd: meta.cwd.clone(),
                branch: None,
                started_at: meta.started_at,
                summary: meta.summary.clone(),
                first_prompt: meta.first_prompt.clone(),
                message_count: 0,
                match_count: 0,
                files_touched: Vec::new(),
                tools_used: Vec::new(),
                matches: Vec::new(),
                export_command: export_command(&meta.tool_slug, &meta.id),
            });
            continue;
        }
        if scanned >= q.scan {
            truncated = true;
            break;
        }
        let Some(adapter) = adapters::adapter_for_tool(&meta.tool_slug) else {
            continue;
        };
        // A transcript that fails to parse is skipped, not fatal — one corrupt
        // file must not sink a search across every tool.
        let Ok(session) = adapter.load_session(&meta.id) else {
            continue;
        };
        scanned += 1;

        if let Some(f) = &q.file
            && !session
                .files_touched
                .iter()
                .any(|p| contains_ci(Some(p), f))
        {
            continue;
        }
        if let Some(t) = &q.tool_used {
            let used = if session.tools_used.is_empty() {
                session.compute_tools_used()
            } else {
                session.tools_used.clone()
            };
            if !used.iter().any(|u| u.eq_ignore_ascii_case(t)) {
                continue;
            }
        }

        // Per-message hits are only meaningful for a text query; --file and
        // --tool-used qualify the session as a whole.
        let all: Vec<MessageMatch> = if matcher.is_noop() {
            Vec::new()
        } else {
            session
                .messages
                .iter()
                .enumerate()
                .filter_map(|(i, m)| match_message(i, m, &matcher, q))
                .collect()
        };
        if all.is_empty() && !matcher.is_noop() {
            continue;
        }

        let count = all.len();
        let shown = all.into_iter().take(q.hits_per_session).collect();
        results.push(to_result(&session, shown, count));
    }

    let matched = results.len();
    Ok(SearchReport {
        candidates: candidates.len(),
        scanned,
        matched,
        truncated,
        sources,
        results,
    })
}

/// Print a search report: one JSON document, or a scannable human summary.
pub fn print_report(report: &SearchReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    if report.results.is_empty() {
        println!(
            "no sessions matched — {} candidate(s), {} transcript(s) scanned across {}{}",
            report.candidates,
            report.scanned,
            if report.sources.is_empty() {
                "no available sources".to_string()
            } else {
                report.sources.join(", ")
            },
            if report.truncated {
                " (stopped early; raise --scan)"
            } else {
                ""
            }
        );
        return Ok(());
    }

    for r in &report.results {
        let date = r
            .started_at
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".into());
        let where_ = r
            .project
            .clone()
            .or_else(|| {
                r.cwd
                    .as_deref()
                    .and_then(|c| c.rsplit('/').next())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "-".into());
        println!(
            "{} {}  {}  {}",
            console::style(&r.tool).cyan(),
            date,
            console::style(&where_).bold(),
            console::style(&r.id).dim()
        );
        if let Some(title) = r.summary.as_deref().or(r.first_prompt.as_deref()) {
            println!("  {}", console::style(one_line(title, 100)).italic());
        }
        for m in &r.matches {
            println!(
                "  {:<9} #{:<4} {}",
                console::style(role_label(m.role)).dim(),
                m.index,
                m.excerpt
            );
        }
        if r.match_count > r.matches.len() {
            println!(
                "  {}",
                console::style(format!(
                    "… {} more match(es); raise --hits to see them",
                    r.match_count - r.matches.len()
                ))
                .dim()
            );
        }
        println!("  {}", console::style(&r.export_command).dim());
        println!();
    }

    println!(
        "{} session(s) matched — {} candidate(s), {} transcript(s) scanned{}",
        report.matched,
        report.candidates,
        report.scanned,
        if report.truncated {
            " (stopped early; raise --limit or --scan)"
        } else {
            ""
        }
    );
    Ok(())
}

fn role_label(role: RoleIR) -> &'static str {
    match role {
        RoleIR::User => "user",
        RoleIR::Assistant => "assistant",
        RoleIR::System => "system",
        RoleIR::Tool => "tool",
    }
}

pub fn one_line(s: &str, width: usize) -> String {
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= width {
        return collapsed;
    }
    collapsed.chars().take(width).chain(['…']).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: RoleIR, text: &str) -> MessageIR {
        MessageIR {
            role,
            content: vec![ContentBlockIR::Text {
                text: text.to_string(),
            }],
            timestamp: None,
        }
    }

    #[test]
    fn role_aliases() {
        assert_eq!(parse_role("human").unwrap(), RoleIR::User);
        assert_eq!(parse_role("AI").unwrap(), RoleIR::Assistant);
        assert!(parse_role("nobody").is_err());
    }

    #[test]
    fn absolute_and_relative_times() {
        let d = parse_time("2026-01-31").unwrap();
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-01-31");
        let ago = parse_time("7d").unwrap();
        assert!((Utc::now() - ago).num_days() == 7);
        assert!(parse_time("whenever").is_err());
    }

    #[test]
    fn role_filter_selects_user_messages_only() {
        let q = SearchQuery {
            text: Some("deploy".into()),
            roles: vec![RoleIR::User],
            ..Default::default()
        };
        let matcher = Matcher::build(&q).unwrap();
        assert!(match_message(0, &msg(RoleIR::User, "deploy the thing"), &matcher, &q).is_some());
        assert!(
            match_message(1, &msg(RoleIR::Assistant, "deploying now"), &matcher, &q).is_none(),
            "assistant message must not match a --role user search"
        );
    }

    #[test]
    fn role_without_a_query_is_rejected_rather_than_matching_everything() {
        let q = SearchQuery {
            roles: vec![RoleIR::User],
            ..Default::default()
        };
        let err = search(&q).unwrap_err().to_string();
        assert!(err.contains("--role narrows"), "got: {err}");
    }

    #[test]
    fn substring_search_is_literal_by_default() {
        let q = SearchQuery {
            text: Some("a.c".into()),
            ..Default::default()
        };
        let matcher = Matcher::build(&q).unwrap();
        assert!(
            matcher.find("abc").is_none(),
            "'.' must not act as a wildcard"
        );
        assert!(matcher.find("a.c").is_some());
    }

    #[test]
    fn regex_mode_opts_into_patterns() {
        let q = SearchQuery {
            text: Some(r"cargo\s+build".into()),
            regex: true,
            ..Default::default()
        };
        let matcher = Matcher::build(&q).unwrap();
        assert!(matcher.find("run cargo  build now").is_some());
    }

    #[test]
    fn tool_use_blocks_are_searchable() {
        let q = SearchQuery {
            text: Some("Bash".into()),
            ..Default::default()
        };
        let matcher = Matcher::build(&q).unwrap();
        let m = MessageIR {
            role: RoleIR::Assistant,
            content: vec![ContentBlockIR::ToolUse {
                name: "Bash".into(),
                input: "ls -la".into(),
                id: None,
            }],
            timestamp: None,
        };
        let hit = match_message(0, &m, &matcher, &q).unwrap();
        assert_eq!(hit.kind, "tool_use");
        assert_eq!(hit.tool.as_deref(), Some("Bash"));
    }

    #[test]
    fn excerpt_stays_within_width_and_marks_elision() {
        let text = collapse(&"word ".repeat(200));
        let out = excerpt(&text, 0, 40, false);
        assert!(out.chars().count() <= 42, "excerpt too wide: {out}");
        assert!(out.ends_with('…'));
        assert_eq!(excerpt("short text", 0, 40, false), "short text");
    }

    #[test]
    fn excerpt_window_surrounds_a_late_match() {
        let text = collapse(&format!(
            "{} NEEDLE {}",
            "pad ".repeat(300),
            "tail ".repeat(300)
        ));
        let at = text.find("NEEDLE").unwrap();
        let out = excerpt(&text, at, 60, false);
        assert!(
            out.contains("NEEDLE"),
            "match fell outside the window: {out}"
        );
        assert!(out.starts_with('…') && out.ends_with('…'));
    }
}
