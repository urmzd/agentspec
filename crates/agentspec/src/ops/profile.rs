//! `agentspec profile`: group skills and agents, and turn the groups off.
//!
//! Turning a profile off does not remove anything from the store. It takes
//! the tool links of its resources away and keeps their records, marked
//! `suspended`, so turning it back on restores exactly the links that were
//! there. See `crate::profiles` for the model.

use std::collections::BTreeMap;
use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{Config, LinkStrategy, SourceType, TrackedKind, hash_resource};
use crate::ops::{agent_render, link, refresh};
use crate::profiles::{self, Profile, ProfilesFile};

/// What applying the current profile state changed on disk.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Links taken out of tool directories.
    pub suspended: usize,
    /// Links put back.
    pub restored: usize,
    /// Copies left in place because they were edited there.
    pub kept_edited: usize,
}

/// Make tool directories match the profile state. Idempotent.
pub fn apply(cfg: &mut Config) -> Result<Applied> {
    let file = ProfilesFile::load()?;
    apply_with(cfg, &file, &config::agents_base_dir())
}

pub(crate) fn apply_with(cfg: &mut Config, file: &ProfilesFile, base: &Path) -> Result<Applied> {
    let mut out = Applied::default();
    let state = cfg.profile_state.clone();

    for resource in &mut cfg.resources {
        let kind = resource.kind;
        if !matches!(kind, TrackedKind::Skill | TrackedKind::Agent) {
            continue;
        }
        let active = file.is_active(kind, &resource.name, &state);
        let store_path = base.join(&resource.path);
        let origin = (resource.source_type == SourceType::Local).then(|| resource.source.clone());

        for link in &mut resource.links {
            // The tool-directory origin of a local resource is the user's own
            // file. It is never removed.
            if origin.as_deref() == Some(link.path.as_str()) {
                continue;
            }
            let path = Path::new(&link.path);
            let on_disk = path.exists() || path.is_symlink();

            if !active && !link.suspended {
                if on_disk {
                    if path.is_symlink() {
                        std::fs::remove_file(path)?;
                    } else if copy_is_untouched(kind, &store_path, link) {
                        if path.is_dir() {
                            std::fs::remove_dir_all(path)?;
                        } else {
                            std::fs::remove_file(path)?;
                        }
                    } else {
                        // Edited in place: `manage verify` reports it. Deleting
                        // it here would lose the edit.
                        out.kept_edited += 1;
                        continue;
                    }
                }
                link.suspended = true;
                out.suspended += 1;
            } else if active && link.suspended {
                if !on_disk {
                    if !store_path.exists() {
                        continue;
                    }
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    match link.strategy {
                        LinkStrategy::Symlink => {
                            let target = link::make_relative(path, &store_path);
                            std::os::unix::fs::symlink(target, path)?;
                        }
                        LinkStrategy::Copy => refresh::write_copy(kind, &store_path, link)?,
                    }
                }
                link.suspended = false;
                out.restored += 1;
            }
        }
    }
    Ok(out)
}

/// True when a copy still holds what agentspec wrote, so removing it loses
/// nothing that is not in the store.
fn copy_is_untouched(
    kind: TrackedKind,
    store_path: &Path,
    link: &crate::inventory::ResourceLink,
) -> bool {
    let Ok(copy_hash) = hash_resource(kind, Path::new(&link.path)) else {
        return false;
    };
    if link.hash.as_deref() == Some(copy_hash.as_str()) {
        return true;
    }
    let expected = if kind == TrackedKind::Agent {
        agent_render::expected_hash(store_path, &link.tool)
    } else {
        hash_resource(kind, store_path)
    };
    expected.is_ok_and(|e| e == copy_hash)
}

// ---------------------------------------------------------------------------
// Member resolution
// ---------------------------------------------------------------------------

/// Resolve a member argument to a managed skill or agent. `skill:<name>` and
/// `agent:<name>` pick a kind when one name is both.
fn resolve_member(cfg: &Config, arg: &str) -> Result<(TrackedKind, String)> {
    if let Some(name) = arg.strip_prefix("skill:") {
        return cfg
            .find(name, TrackedKind::Skill)
            .map(|_| (TrackedKind::Skill, name.to_string()))
            .ok_or_else(|| AppError::SkillNotFound(name.into()));
    }
    if let Some(name) = arg.strip_prefix("agent:") {
        return cfg
            .find(name, TrackedKind::Agent)
            .map(|_| (TrackedKind::Agent, name.to_string()))
            .ok_or_else(|| AppError::AgentNotFound(name.into()));
    }
    let skill = cfg.find(arg, TrackedKind::Skill).is_some();
    let agent = cfg.find(arg, TrackedKind::Agent).is_some();
    match (skill, agent) {
        (true, false) => Ok((TrackedKind::Skill, arg.to_string())),
        (false, true) => Ok((TrackedKind::Agent, arg.to_string())),
        (true, true) => Err(AppError::Other(format!(
            "'{arg}' is both a skill and an agent: write skill:{arg} or agent:{arg}"
        ))),
        (false, false) => Err(AppError::Other(format!(
            "'{arg}' is not a managed skill or agent (see `agentspec manage list`)"
        ))),
    }
}

fn resolve_members(cfg: &Config, args: &[String]) -> Result<Vec<(TrackedKind, String)>> {
    args.iter().map(|a| resolve_member(cfg, a)).collect()
}

fn require<'a>(file: &'a ProfilesFile, name: &str) -> Result<&'a Profile> {
    file.profiles.get(name).ok_or_else(|| {
        AppError::Other(format!(
            "no profile named '{name}' (see `agentspec profile list`)"
        ))
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

pub fn list(cfg: &Config, json: bool) -> Result<()> {
    let file = ProfilesFile::load()?;

    if json {
        let out: Vec<serde_json::Value> = file
            .profiles
            .iter()
            .map(|(name, p)| profile_json(name, p, &file, &cfg.profile_state))
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if file.profiles.is_empty() {
        println!("  No profiles. Create one with `agentspec profile create <name> <skill>...`");
        return Ok(());
    }

    println!(
        "  {:<5} {:<20} {:>6} {:>6}  {}",
        style("State").bold().underlined(),
        style("Profile").bold().underlined(),
        style("Skills").bold().underlined(),
        style("Agents").bold().underlined(),
        style("Description").bold().underlined(),
    );
    for (name, p) in &file.profiles {
        let on = file.is_on(name, &cfg.profile_state);
        println!(
            "  {:<5} {:<20} {:>6} {:>6}  {}",
            if on {
                style("on").green()
            } else {
                style("off").dim()
            },
            name,
            p.skills.len(),
            p.agents.len(),
            p.description.as_deref().unwrap_or(""),
        );
    }

    let (in_profiles, off) = cfg
        .resources
        .iter()
        .filter(|r| matches!(r.kind, TrackedKind::Skill | TrackedKind::Agent))
        .fold((0usize, 0usize), |(grouped, off), r| {
            let grouped_now = !file.profiles_of(r.kind, &r.name).is_empty();
            let active = file.is_active(r.kind, &r.name, &cfg.profile_state);
            (
                grouped + usize::from(grouped_now),
                off + usize::from(!active),
            )
        });
    println!(
        "\n  {in_profiles} resource(s) in profiles, {off} currently off. Resources in no profile are always linked."
    );
    Ok(())
}

fn profile_json(
    name: &str,
    p: &Profile,
    file: &ProfilesFile,
    state: &BTreeMap<String, bool>,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "on": file.is_on(name, state),
        "default": p.enabled,
        "description": p.description,
        "skills": p.skills,
        "agents": p.agents,
    })
}

pub fn show(cfg: &Config, name: &str, json: bool) -> Result<()> {
    let file = ProfilesFile::load()?;
    let p = require(&file, name)?;
    let on = file.is_on(name, &cfg.profile_state);

    let member = |kind: TrackedKind, member: &str| {
        let tracked = cfg.find(member, kind);
        let tools: Vec<String> = tracked
            .map(|r| {
                r.links
                    .iter()
                    .filter(|l| !l.suspended)
                    .map(|l| l.tool.clone())
                    .collect()
            })
            .unwrap_or_default();
        serde_json::json!({
            "name": member,
            "kind": kind.to_string(),
            "managed": tracked.is_some(),
            "active": file.is_active(kind, member, &cfg.profile_state),
            "linked_to": tools,
            "also_in": file
                .profiles_of(kind, member)
                .into_iter()
                .filter(|other| *other != name)
                .collect::<Vec<_>>(),
        })
    };
    let members: Vec<serde_json::Value> = p
        .skills
        .iter()
        .map(|m| member(TrackedKind::Skill, m))
        .chain(p.agents.iter().map(|m| member(TrackedKind::Agent, m)))
        .collect();

    if json {
        let mut out = profile_json(name, p, &file, &cfg.profile_state);
        out["members"] = serde_json::Value::Array(members);
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!(
        "  {} {}  {}",
        style(name).bold(),
        if on {
            style("on").green()
        } else {
            style("off").dim()
        },
        p.description.as_deref().unwrap_or(""),
    );
    if members.is_empty() {
        println!("  (no members)");
    }
    for m in &members {
        let note = if !m["managed"].as_bool().unwrap_or(false) {
            "not installed".to_string()
        } else if !m["active"].as_bool().unwrap_or(false) {
            "off".to_string()
        } else {
            let tools: Vec<&str> = m["linked_to"]
                .as_array()
                .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
                .unwrap_or_default();
            if tools.is_empty() {
                "active, not linked to any tool".to_string()
            } else {
                tools.join(", ")
            }
        };
        let also: Vec<&str> = m["also_in"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
            .unwrap_or_default();
        println!(
            "  {:<6} {:<32} {}{}",
            m["kind"].as_str().unwrap_or(""),
            m["name"].as_str().unwrap_or(""),
            note,
            if also.is_empty() {
                String::new()
            } else {
                format!("  (also in {})", also.join(", "))
            }
        );
    }
    Ok(())
}

pub fn create(
    cfg: &mut Config,
    name: &str,
    description: Option<String>,
    off: bool,
    members: &[String],
    json: bool,
) -> Result<()> {
    profiles::validate_name(name)?;
    let mut file = ProfilesFile::load()?;
    if file.profiles.contains_key(name) {
        return Err(AppError::AlreadyExists(format!("profile '{name}'")));
    }
    let mut profile = Profile {
        description,
        enabled: !off,
        ..Profile::default()
    };
    for (kind, member) in resolve_members(cfg, members)? {
        if let Some(list) = profile.members_mut(kind)
            && !list.contains(&member)
        {
            list.push(member);
        }
    }
    file.profiles.insert(name.to_string(), profile);
    file.save()?;
    // A machine choice left over from an earlier profile of this name must
    // not override what was just asked for.
    cfg.profile_state.remove(name);
    finish(cfg, &file, &format!("Created profile '{name}'"), json)
}

pub fn add(cfg: &mut Config, name: &str, members: &[String], json: bool) -> Result<()> {
    let mut file = ProfilesFile::load()?;
    require(&file, name)?;
    let resolved = resolve_members(cfg, members)?;
    let profile = file.profiles.get_mut(name).expect("checked above");
    let mut added = 0;
    for (kind, member) in resolved {
        if let Some(list) = profile.members_mut(kind)
            && !list.contains(&member)
        {
            list.push(member);
            added += 1;
        }
    }
    file.save()?;
    finish(cfg, &file, &format!("Added {added} to '{name}'"), json)
}

pub fn remove(cfg: &mut Config, name: &str, members: &[String], json: bool) -> Result<()> {
    let mut file = ProfilesFile::load()?;
    require(&file, name)?;
    let profile = file.profiles.get_mut(name).expect("checked above");
    let mut removed = 0;
    for arg in members {
        // A member may name a resource that is no longer installed, so match
        // on the name instead of resolving it against the inventory.
        let (kinds, member): (&[TrackedKind], &str) = if let Some(m) = arg.strip_prefix("skill:") {
            (&[TrackedKind::Skill], m)
        } else if let Some(m) = arg.strip_prefix("agent:") {
            (&[TrackedKind::Agent], m)
        } else {
            (&[TrackedKind::Skill, TrackedKind::Agent], arg.as_str())
        };
        for kind in kinds {
            if let Some(list) = profile.members_mut(*kind) {
                let before = list.len();
                list.retain(|m| m != member);
                removed += before - list.len();
            }
        }
    }
    file.save()?;
    finish(
        cfg,
        &file,
        &format!("Removed {removed} from '{name}'"),
        json,
    )
}

pub fn delete(cfg: &mut Config, name: &str, json: bool) -> Result<()> {
    let mut file = ProfilesFile::load()?;
    require(&file, name)?;
    file.profiles.remove(name);
    file.save()?;
    cfg.profile_state.remove(name);
    finish(cfg, &file, &format!("Deleted profile '{name}'"), json)
}

/// Turn profiles on or off on this machine, then apply.
pub fn set(cfg: &mut Config, names: &[String], on: bool, json: bool) -> Result<()> {
    let file = ProfilesFile::load()?;
    for name in names {
        require(&file, name)?;
    }
    for name in names {
        cfg.profile_state.insert(name.clone(), on);
    }
    let what = format!(
        "Turned {} {}",
        if on { "on" } else { "off" },
        names.join(", ")
    );
    finish(cfg, &file, &what, json)
}

pub fn apply_command(cfg: &mut Config, json: bool) -> Result<()> {
    let file = ProfilesFile::load()?;
    finish(cfg, &file, "Applied profiles", json)
}

/// Apply the profile state and report what changed.
fn finish(cfg: &mut Config, file: &ProfilesFile, what: &str, json: bool) -> Result<()> {
    let applied = apply_with(cfg, file, &config::agents_base_dir())?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "links_suspended": applied.suspended,
                "links_restored": applied.restored,
                "edited_copies_kept": applied.kept_edited,
                "profiles": file
                    .profiles
                    .iter()
                    .map(|(n, p)| profile_json(n, p, file, &cfg.profile_state))
                    .collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    println!("  {} {what}", style("✓").green().bold());
    if applied.suspended > 0 {
        println!(
            "  {} Removed {} tool link(s). The resources stay in the store",
            style("~").yellow(),
            applied.suspended
        );
    }
    if applied.restored > 0 {
        println!(
            "  {} Restored {} tool link(s)",
            style("✓").green().bold(),
            applied.restored
        );
    }
    if applied.kept_edited > 0 {
        println!(
            "  {} Left {} copy(ies) in place because they were edited there. See `agentspec manage verify`",
            style("⚠").yellow().bold(),
            applied.kept_edited
        );
    }
    Ok(())
}

/// The profiles keeping a resource off, for messages. Empty when it is active.
pub fn blocking_profiles(cfg: &Config, kind: TrackedKind, name: &str) -> Vec<String> {
    let Ok(file) = ProfilesFile::load() else {
        return Vec::new();
    };
    if file.is_active(kind, name, &cfg.profile_state) {
        return Vec::new();
    }
    file.profiles_of(kind, name)
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ResourceLink, TrackedResource};

    fn write_skill(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: s\ndescription: d\n---\n{body}\n"),
        )
        .unwrap();
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        base: std::path::PathBuf,
        tool: std::path::PathBuf,
        cfg: Config,
    }

    /// Two store skills, `web-a` and `loose`, each with a symlink in one tool
    /// directory and a copy in another.
    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let tool = tmp.path().join("tools");
        let mut cfg = Config::empty();
        for name in ["web-a", "loose"] {
            let store = base.join("skills").join(name);
            write_skill(&store, "v1");
            let mut r = TrackedResource::new(
                name.into(),
                TrackedKind::Skill,
                "discovered".into(),
                SourceType::Discovered,
                format!("skills/{name}"),
                hash_resource(TrackedKind::Skill, &store).unwrap(),
            );
            let sym = tool.join("sym").join(name);
            std::fs::create_dir_all(sym.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(link::make_relative(&sym, &store), &sym).unwrap();
            r.links.push(ResourceLink::written(
                TrackedKind::Skill,
                "claude-code",
                LinkStrategy::Symlink,
                &sym,
            ));
            let copy = tool.join("copy").join(name);
            write_skill(&copy, "v1");
            r.links.push(ResourceLink::written(
                TrackedKind::Skill,
                "opencode",
                LinkStrategy::Copy,
                &copy,
            ));
            cfg.add(r);
        }
        Fixture {
            _tmp: tmp,
            base,
            tool,
            cfg,
        }
    }

    fn web(enabled: bool) -> ProfilesFile {
        let mut file = ProfilesFile::default();
        file.profiles.insert(
            "web".into(),
            Profile {
                enabled,
                skills: vec!["web-a".into()],
                ..Profile::default()
            },
        );
        file
    }

    #[test]
    fn turning_a_profile_off_removes_only_its_links_and_on_restores_them() {
        let mut fx = fixture();
        let sym = fx.tool.join("sym/web-a");
        let copy = fx.tool.join("copy/web-a");

        let out = apply_with(&mut fx.cfg, &web(false), &fx.base).unwrap();
        assert_eq!(out.suspended, 2);
        assert!(!sym.is_symlink() && !copy.exists());
        // The store and the resource outside the profile are untouched.
        assert!(fx.base.join("skills/web-a/SKILL.md").exists());
        assert!(fx.tool.join("sym/loose").is_symlink());
        assert!(fx.tool.join("copy/loose/SKILL.md").exists());
        // Records are kept so the links can come back.
        let links = &fx.cfg.find("web-a", TrackedKind::Skill).unwrap().links;
        assert!(links.iter().all(|l| l.suspended));

        // A second run changes nothing.
        assert_eq!(
            apply_with(&mut fx.cfg, &web(false), &fx.base).unwrap(),
            Applied::default()
        );

        let out = apply_with(&mut fx.cfg, &web(true), &fx.base).unwrap();
        assert_eq!(out.restored, 2);
        assert!(sym.is_symlink());
        assert!(
            sym.join("SKILL.md").exists(),
            "symlink resolves to the store"
        );
        assert!(copy.join("SKILL.md").exists());
        let links = &fx.cfg.find("web-a", TrackedKind::Skill).unwrap().links;
        assert!(links.iter().all(|l| !l.suspended));
    }

    #[test]
    fn the_machine_choice_overrides_the_file_default() {
        let mut fx = fixture();
        fx.cfg.profile_state.insert("web".into(), false);
        // The file says on, this machine says off.
        let out = apply_with(&mut fx.cfg, &web(true), &fx.base).unwrap();
        assert_eq!(out.suspended, 2);
    }

    #[test]
    fn a_copy_edited_in_place_is_kept_when_its_profile_goes_off() {
        let mut fx = fixture();
        let copy = fx.tool.join("copy/web-a");
        write_skill(&copy, "edited by hand");

        let out = apply_with(&mut fx.cfg, &web(false), &fx.base).unwrap();
        assert_eq!(out.kept_edited, 1);
        assert_eq!(out.suspended, 1, "the symlink still goes");
        assert!(copy.join("SKILL.md").exists());
    }

    #[test]
    fn a_resource_in_two_profiles_stays_linked_while_one_is_on() {
        let mut fx = fixture();
        let mut file = web(false);
        file.profiles.insert(
            "other".into(),
            Profile {
                skills: vec!["web-a".into()],
                ..Profile::default()
            },
        );
        assert_eq!(
            apply_with(&mut fx.cfg, &file, &fx.base).unwrap(),
            Applied::default()
        );
        assert!(fx.tool.join("sym/web-a").is_symlink());
    }

    #[test]
    fn deleting_a_profile_brings_its_resources_back() {
        let mut fx = fixture();
        apply_with(&mut fx.cfg, &web(false), &fx.base).unwrap();
        let out = apply_with(&mut fx.cfg, &ProfilesFile::default(), &fx.base).unwrap();
        assert_eq!(out.restored, 2);
    }

    #[test]
    fn members_resolve_by_name_and_reject_unknown_or_ambiguous_ones() {
        let fx = fixture();
        assert_eq!(
            resolve_member(&fx.cfg, "web-a").unwrap(),
            (TrackedKind::Skill, "web-a".to_string())
        );
        assert_eq!(
            resolve_member(&fx.cfg, "skill:web-a").unwrap(),
            (TrackedKind::Skill, "web-a".to_string())
        );
        assert!(resolve_member(&fx.cfg, "nope").is_err());
        assert!(resolve_member(&fx.cfg, "agent:web-a").is_err());

        let mut cfg = fx.cfg.clone();
        cfg.add(TrackedResource::new(
            "web-a".into(),
            TrackedKind::Agent,
            "discovered".into(),
            SourceType::Discovered,
            "agents/web-a.md".into(),
            "sha256:x".into(),
        ));
        let err = resolve_member(&cfg, "web-a").unwrap_err().to_string();
        assert!(err.contains("both a skill and an agent"), "{err}");
    }
}
