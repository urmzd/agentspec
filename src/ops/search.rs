use console::style;
use serde::Deserialize;

use crate::error::Result;

#[derive(Deserialize)]
struct SearchResponse {
    items: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    full_name: String,
    description: Option<String>,
    html_url: String,
    stargazers_count: u64,
}

pub async fn search(query: &str, limit: usize, json: bool) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/search/repositories?q={query}+topic:agent-skill&sort=stars&per_page={limit}"
    );

    let resp: SearchResponse = client
        .get(&url)
        .header("User-Agent", "agentctl")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?
        .json()
        .await?;

    if json {
        let out: Vec<serde_json::Value> = resp
            .items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "name": item.full_name,
                    "description": item.description,
                    "url": item.html_url,
                    "stars": item.stargazers_count,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if resp.items.is_empty() {
        println!("  No results for '{query}'");
        return Ok(());
    }

    println!(
        "  {:<35} {:<5} {}",
        style("Repository").bold().underlined(),
        style("Stars").bold().underlined(),
        style("Description").bold().underlined()
    );

    for item in &resp.items {
        let desc = item
            .description
            .as_deref()
            .map(|d| truncate(d, 45))
            .unwrap_or_default();
        println!(
            "  {:<35} {:<5} {}",
            style(&item.full_name).cyan(),
            item.stargazers_count,
            desc
        );
    }

    println!(
        "\n  Install with: {} agentctl skill install <repo>",
        style("$").dim()
    );
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
