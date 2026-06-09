use std::collections::HashMap;

use console::style;

use crate::error::Result;
use crate::inventory::{Config, TrackedKind};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DuplicateMember {
    name: String,
    kind: TrackedKind,
    location: String, // human-readable location
    managed: bool,
}

#[derive(Debug)]
struct DuplicateGroup {
    key: String, // hash or name
    members: Vec<DuplicateMember>,
}

// ---------------------------------------------------------------------------
// Dedup logic
// ---------------------------------------------------------------------------

fn find_hash_duplicates(cfg: &Config) -> Vec<DuplicateGroup> {
    // Collect all resources with their hashes: (hash, member)
    let mut by_hash: HashMap<String, Vec<DuplicateMember>> = HashMap::new();

    // Managed resources
    for r in &cfg.resources {
        if r.hash.is_empty() {
            continue;
        }
        let location = if r.links.is_empty() {
            format!("shared-store ({})", r.path)
        } else {
            r.links
                .iter()
                .map(|l| l.tool.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        by_hash
            .entry(r.hash.clone())
            .or_default()
            .push(DuplicateMember {
                name: r.name.clone(),
                kind: r.kind,
                location,
                managed: true,
            });
    }

    // Discovered (unmanaged) resources
    for d in &cfg.discovered {
        let hash = match &d.content_hash {
            Some(h) => h.clone(),
            None => continue,
        };
        let location = d
            .found_in
            .iter()
            .map(|l| format!("{} ({})", l.tool, l.path))
            .collect::<Vec<_>>()
            .join(", ");
        by_hash.entry(hash).or_default().push(DuplicateMember {
            name: d.name.clone(),
            kind: d.kind,
            location,
            managed: false,
        });
    }

    // Only keep groups with >1 member
    by_hash
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|(key, members)| DuplicateGroup { key, members })
        .collect()
}

fn find_name_duplicates(cfg: &Config) -> Vec<DuplicateGroup> {
    // Group all resources by (name, kind)
    let mut by_name: HashMap<String, Vec<DuplicateMember>> = HashMap::new();

    for r in &cfg.resources {
        let key = format!("{}/{}", r.kind, r.name);
        let location = if r.links.is_empty() {
            format!("shared-store ({})", r.path)
        } else {
            r.links
                .iter()
                .map(|l| l.tool.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        by_name.entry(key).or_default().push(DuplicateMember {
            name: r.name.clone(),
            kind: r.kind,
            location,
            managed: true,
        });
    }

    for d in &cfg.discovered {
        let key = format!("{}/{}", d.kind, d.name);
        // Each found_in location is a separate instance
        for loc in &d.found_in {
            by_name
                .entry(key.clone())
                .or_default()
                .push(DuplicateMember {
                    name: d.name.clone(),
                    kind: d.kind,
                    location: format!("{} ({})", loc.tool, loc.path),
                    managed: false,
                });
        }
    }

    by_name
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|(key, members)| DuplicateGroup { key, members })
        .collect()
}

// ---------------------------------------------------------------------------
// CLI command
// ---------------------------------------------------------------------------

pub fn dedup(cfg: &Config, by_hash: bool, by_name: bool, json: bool) -> Result<()> {
    // Default: show both
    let show_hash = by_hash || !by_name;
    let show_name = by_name || !by_hash;

    let mut hash_groups = if show_hash {
        find_hash_duplicates(cfg)
    } else {
        Vec::new()
    };
    let mut name_groups = if show_name {
        find_name_duplicates(cfg)
    } else {
        Vec::new()
    };

    hash_groups.sort_by(|a, b| a.key.cmp(&b.key));
    name_groups.sort_by(|a, b| a.key.cmp(&b.key));

    if json {
        let out = serde_json::json!({
            "by_hash": hash_groups.iter().map(|g| {
                serde_json::json!({
                    "hash": g.key,
                    "members": g.members.iter().map(|m| {
                        serde_json::json!({
                            "name": m.name,
                            "kind": format!("{}", m.kind),
                            "location": m.location,
                            "managed": m.managed,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "by_name": name_groups.iter().map(|g| {
                serde_json::json!({
                    "name": g.key,
                    "members": g.members.iter().map(|m| {
                        serde_json::json!({
                            "location": m.location,
                            "managed": m.managed,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let mut total = 0;

    if show_hash && !hash_groups.is_empty() {
        println!(
            "  {}",
            style("Content Duplicates (same hash)").bold().underlined()
        );
        for group in &hash_groups {
            let short_hash = if group.key.len() > 20 {
                format!("{}...", &group.key[..20])
            } else {
                group.key.clone()
            };
            println!("  {} {}", style("hash:").dim(), style(&short_hash).dim());
            for m in &group.members {
                let status = if m.managed {
                    style("managed").green()
                } else {
                    style("unmanaged").yellow()
                };
                println!(
                    "    {} {:<25} {:<10} [{}] {}",
                    if m.managed {
                        style("*").green()
                    } else {
                        style("-").dim()
                    },
                    m.name,
                    format!("{}", m.kind),
                    status,
                    style(&m.location).dim(),
                );
            }
            println!();
            total += group.members.len() - 1; // extra copies
        }
    }

    if show_name && !name_groups.is_empty() {
        println!(
            "  {}",
            style("Name Duplicates (same name in multiple locations)")
                .bold()
                .underlined()
        );
        for group in &name_groups {
            println!("  {}", style(&group.key).cyan());
            for m in &group.members {
                let status = if m.managed {
                    style("managed").green()
                } else {
                    style("unmanaged").yellow()
                };
                println!(
                    "    {} [{}] {}",
                    if m.managed {
                        style("*").green()
                    } else {
                        style("-").dim()
                    },
                    status,
                    style(&m.location).dim(),
                );
            }
            println!();
            total += group.members.len() - 1;
        }
    }

    if hash_groups.is_empty() && name_groups.is_empty() {
        println!("  {} No duplicates found", style("~").green().bold());
    } else {
        println!(
            "  {} duplicate instance(s) found. Use `agentspec manage add <name>` to consolidate.",
            style(total).bold()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{DiscoveredResource, DiscoveryLocation, SourceType, TrackedResource};

    fn managed(name: &str, hash: &str) -> TrackedResource {
        TrackedResource::new(
            name.to_string(),
            TrackedKind::Skill,
            "discovered".to_string(),
            SourceType::Discovered,
            format!("skills/{name}"),
            hash.to_string(),
        )
    }

    fn found(name: &str, hash: Option<&str>, paths: &[&str]) -> DiscoveredResource {
        DiscoveredResource {
            name: name.to_string(),
            kind: TrackedKind::Skill,
            found_in: paths
                .iter()
                .map(|p| DiscoveryLocation {
                    tool: "claude-code".to_string(),
                    path: p.to_string(),
                })
                .collect(),
            content_hash: hash.map(String::from),
        }
    }

    #[test]
    fn hash_duplicates_group_managed_and_discovered_copies() {
        let mut cfg = Config::empty();
        cfg.resources.push(managed("alpha", "sha256:dead"));
        cfg.discovered
            .push(found("alpha-copy", Some("sha256:dead"), &["/tools/a"]));
        cfg.discovered
            .push(found("unique", Some("sha256:beef"), &["/tools/b"]));

        let groups = find_hash_duplicates(&cfg);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "sha256:dead");
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(groups[0].members.iter().filter(|m| m.managed).count(), 1);
    }

    #[test]
    fn hash_duplicates_ignore_resources_without_hashes() {
        let mut cfg = Config::empty();
        cfg.resources.push(managed("no-hash-a", ""));
        cfg.resources.push(managed("no-hash-b", ""));
        cfg.discovered.push(found("no-hash-c", None, &["/a"]));
        cfg.discovered.push(found("no-hash-d", None, &["/b"]));
        assert!(find_hash_duplicates(&cfg).is_empty());
    }

    #[test]
    fn name_duplicates_count_each_discovered_location() {
        let mut cfg = Config::empty();
        cfg.resources.push(managed("alpha", "sha256:aaa"));
        cfg.discovered.push(found(
            "alpha",
            Some("sha256:bbb"),
            &["/tools/a", "/tools/b"],
        ));

        let groups = find_name_duplicates(&cfg);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "skill/alpha");
        assert_eq!(groups[0].members.len(), 3);
    }

    #[test]
    fn no_duplicates_for_distinct_resources() {
        let mut cfg = Config::empty();
        cfg.resources.push(managed("alpha", "sha256:aaa"));
        cfg.discovered
            .push(found("beta", Some("sha256:bbb"), &["/tools/a"]));
        assert!(find_hash_duplicates(&cfg).is_empty());
        assert!(find_name_duplicates(&cfg).is_empty());
    }
}
