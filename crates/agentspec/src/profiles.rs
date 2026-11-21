//! Profiles: named groups of skills and agents that can be turned off.
//!
//! Every linked skill costs each tool context on every session, because the
//! tool lists its name and description up front. A profile groups resources
//! that belong to one kind of work so they can be unloaded together and
//! brought back when that work starts.
//!
//! Definitions live in `~/.agents/profiles.yml`, next to the resources they
//! name, so they travel with the store. Which profiles are on is a property
//! of the machine and lives in the agentspec config (`Config::profile_state`).
//!
//! A resource is **active** when it belongs to no profile, or to at least one
//! profile that is on. Only active resources are linked into tools.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::TrackedKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

impl Default for ProfilesFile {
    fn default() -> Self {
        Self {
            version: default_version(),
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the profile is on for a machine that has not chosen itself.
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            description: None,
            enabled: true,
            skills: Vec::new(),
            agents: Vec::new(),
        }
    }
}

fn default_version() -> u32 {
    1
}

fn default_enabled() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl Profile {
    pub fn members(&self, kind: TrackedKind) -> &[String] {
        match kind {
            TrackedKind::Skill => &self.skills,
            TrackedKind::Agent => &self.agents,
            _ => &[],
        }
    }

    pub fn members_mut(&mut self, kind: TrackedKind) -> Option<&mut Vec<String>> {
        match kind {
            TrackedKind::Skill => Some(&mut self.skills),
            TrackedKind::Agent => Some(&mut self.agents),
            _ => None,
        }
    }
}

impl ProfilesFile {
    /// Load `~/.agents/profiles.yml`. A missing file is an empty set.
    pub fn load() -> Result<Self> {
        Self::load_from(&config::profiles_file())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(path)?;
        if data.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_yaml_ng::from_str(&data)
            .map_err(|e| AppError::Other(format!("{}: {e}", path.display())))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&config::profiles_file())
    }

    /// Atomic write, so a crash never leaves a truncated file behind.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_yaml_ng::to_string(self)?;
        let tmp = path.with_extension("yml.tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Whether `name` is on: the machine's own choice, else the file default.
    pub fn is_on(&self, name: &str, state: &BTreeMap<String, bool>) -> bool {
        match (state.get(name), self.profiles.get(name)) {
            (Some(on), Some(_)) => *on,
            (None, Some(profile)) => profile.enabled,
            (_, None) => false,
        }
    }

    /// Names of the profiles a resource belongs to.
    pub fn profiles_of(&self, kind: TrackedKind, name: &str) -> Vec<&str> {
        self.profiles
            .iter()
            .filter(|(_, p)| p.members(kind).iter().any(|m| m == name))
            .map(|(n, _)| n.as_str())
            .collect()
    }

    /// A resource is active when it is in no profile, or in one that is on.
    pub fn is_active(&self, kind: TrackedKind, name: &str, state: &BTreeMap<String, bool>) -> bool {
        let owners = self.profiles_of(kind, name);
        owners.is_empty() || owners.iter().any(|p| self.is_on(p, state))
    }
}

/// Profile names are used as YAML keys and CLI arguments.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AppError::Other(format!(
            "invalid profile name '{name}': use lowercase letters, digits, '-' and '_'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> ProfilesFile {
        let mut f = ProfilesFile::default();
        f.profiles.insert(
            "web".into(),
            Profile {
                skills: vec!["style".into(), "shared".into()],
                ..Profile::default()
            },
        );
        f.profiles.insert(
            "release".into(),
            Profile {
                enabled: false,
                skills: vec!["ship".into(), "shared".into()],
                agents: vec!["releaser".into()],
                ..Profile::default()
            },
        );
        f
    }

    #[test]
    fn a_resource_in_no_profile_is_always_active() {
        let f = file();
        assert!(f.is_active(TrackedKind::Skill, "loose", &BTreeMap::new()));
    }

    #[test]
    fn the_file_default_applies_until_the_machine_chooses() {
        let f = file();
        let mut state = BTreeMap::new();
        assert!(f.is_on("web", &state));
        assert!(!f.is_on("release", &state));
        assert!(!f.is_active(TrackedKind::Skill, "ship", &state));
        assert!(!f.is_active(TrackedKind::Agent, "releaser", &state));

        state.insert("release".into(), true);
        assert!(f.is_active(TrackedKind::Skill, "ship", &state));
    }

    #[test]
    fn one_profile_that_is_on_keeps_a_shared_resource_active() {
        let f = file();
        let mut state = BTreeMap::new();
        // web on, release off: "shared" is in both.
        assert!(f.is_active(TrackedKind::Skill, "shared", &state));
        state.insert("web".into(), false);
        assert!(!f.is_active(TrackedKind::Skill, "shared", &state));
    }

    #[test]
    fn skills_and_agents_with_one_name_are_separate_members() {
        let f = file();
        assert!(f.profiles_of(TrackedKind::Skill, "releaser").is_empty());
        assert_eq!(f.profiles_of(TrackedKind::Agent, "releaser"), ["release"]);
    }

    #[test]
    fn state_for_a_deleted_profile_is_ignored() {
        let f = file();
        let mut state = BTreeMap::new();
        state.insert("gone".into(), true);
        assert!(!f.is_on("gone", &state));
    }

    #[test]
    fn save_then_load_round_trips_and_omits_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profiles.yml");
        let f = file();
        f.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // `enabled: true` is the default and is not written.
        assert_eq!(text.matches("enabled").count(), 1);
        assert_eq!(ProfilesFile::load_from(&path).unwrap(), f);
    }

    #[test]
    fn a_missing_or_empty_file_is_an_empty_set() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profiles.yml");
        assert!(ProfilesFile::load_from(&path).unwrap().profiles.is_empty());
        std::fs::write(&path, "\n").unwrap();
        assert!(ProfilesFile::load_from(&path).unwrap().profiles.is_empty());
    }

    #[test]
    fn names_are_restricted_to_safe_characters() {
        assert!(validate_name("web-ui_2").is_ok());
        assert!(validate_name("Web").is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name("").is_err());
    }
}
