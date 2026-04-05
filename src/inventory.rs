use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::config;
use crate::error::{AppError, Result};
use crate::ir::ResourceKind;
use crate::lockfile::LockFile as LegacyLockFile;

// ---------------------------------------------------------------------------
// Config (`<config_dir>/agentspec/config.yml`)
//
// Single source of truth. SHA-256 hashes on each resource ARE the lock —
// any external modification is detected by comparing the stored hash
// against the current content on disk.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scan: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<TrackedResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovered: Vec<DiscoveredResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedResource {
    pub name: String,
    pub kind: TrackedKind,
    pub source: String,
    pub source_type: SourceType,
    /// Relative to `~/.agents/` (e.g. `skills/my-skill` or `agents/my-agent.md`)
    pub path: String,
    /// `sha256:<hex>` — the lock. If the content changes, this won't match.
    pub hash: String,
    pub installed_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<ResourceLink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackedKind {
    Skill,
    Agent,
    Session,
    Memory,
    ProjectConfig,
    LlmsTxt,
}

impl From<ResourceKind> for TrackedKind {
    fn from(k: ResourceKind) -> Self {
        match k {
            ResourceKind::Skill => TrackedKind::Skill,
            ResourceKind::Agent => TrackedKind::Agent,
            ResourceKind::Session => TrackedKind::Session,
            ResourceKind::Memory => TrackedKind::Memory,
            ResourceKind::ProjectConfig => TrackedKind::ProjectConfig,
            ResourceKind::LlmsTxt => TrackedKind::LlmsTxt,
        }
    }
}

impl From<TrackedKind> for ResourceKind {
    fn from(k: TrackedKind) -> Self {
        match k {
            TrackedKind::Skill => ResourceKind::Skill,
            TrackedKind::Agent => ResourceKind::Agent,
            TrackedKind::Session => ResourceKind::Session,
            TrackedKind::Memory => ResourceKind::Memory,
            TrackedKind::ProjectConfig => ResourceKind::ProjectConfig,
            TrackedKind::LlmsTxt => ResourceKind::LlmsTxt,
        }
    }
}

impl std::fmt::Display for TrackedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackedKind::Skill => write!(f, "skill"),
            TrackedKind::Agent => write!(f, "agent"),
            TrackedKind::Session => write!(f, "session"),
            TrackedKind::Memory => write!(f, "memory"),
            TrackedKind::ProjectConfig => write!(f, "project-config"),
            TrackedKind::LlmsTxt => write!(f, "llms-txt"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Git,
    Local,
    Discovered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLink {
    pub tool: String,
    pub strategy: LinkStrategy,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkStrategy {
    Symlink,
    Copy,
}

// ---------------------------------------------------------------------------
// Discovery cache (unmanaged resources found during scan)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredResource {
    pub name: String,
    pub kind: TrackedKind,
    pub found_in: Vec<DiscoveryLocation>,
    /// Content hash for dedup — computed during scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryLocation {
    pub tool: String,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Config methods
// ---------------------------------------------------------------------------

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let data = std::fs::read_to_string(path)?;
        let cfg: Self = serde_yaml::from_str(&data)?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_yaml::to_string(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn empty() -> Self {
        Self {
            version: 1,
            last_scan: None,
            resources: Vec::new(),
            discovered: Vec::new(),
        }
    }

    pub fn find(&self, name: &str, kind: TrackedKind) -> Option<&TrackedResource> {
        self.resources
            .iter()
            .find(|r| r.name == name && r.kind == kind)
    }

    pub fn find_mut(&mut self, name: &str, kind: TrackedKind) -> Option<&mut TrackedResource> {
        self.resources
            .iter_mut()
            .find(|r| r.name == name && r.kind == kind)
    }

    pub fn remove(&mut self, name: &str, kind: TrackedKind) {
        self.resources
            .retain(|r| !(r.name == name && r.kind == kind));
    }

    pub fn add(&mut self, resource: TrackedResource) {
        if let Some(existing) = self.find_mut(&resource.name, resource.kind) {
            *existing = resource;
        } else {
            self.resources.push(resource);
        }
    }

    /// Migrate from the legacy `.skill-lock.json` v3 format.
    pub fn migrate_from_v3(legacy: &LegacyLockFile) -> Self {
        let mut cfg = Self::empty();
        for (name, entry) in &legacy.skills {
            let source_type = match entry.source_type.as_str() {
                "github" => SourceType::Git,
                _ => SourceType::Local,
            };
            cfg.resources.push(TrackedResource {
                name: name.clone(),
                kind: TrackedKind::Skill,
                source: if entry.source_url.is_empty() {
                    entry.source.clone()
                } else {
                    entry.source_url.clone()
                },
                source_type,
                path: format!("skills/{name}"),
                // Legacy hash — will be flagged by verify, user runs --accept to upgrade
                hash: format!("sha1:{}", entry.skill_folder_hash),
                installed_at: entry.installed_at.clone(),
                updated_at: entry.updated_at.clone(),
                links: Vec::new(),
            });
        }
        cfg
    }
}

impl TrackedResource {
    pub fn new(
        name: String,
        kind: TrackedKind,
        source: String,
        source_type: SourceType,
        path: String,
        hash: String,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name,
            kind,
            source,
            source_type,
            path,
            hash,
            installed_at: now.clone(),
            updated_at: now,
            links: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Hashing — the SHA-256 IS the lock
// ---------------------------------------------------------------------------

/// Compute SHA-256 hash of a single file, returned as `sha256:<hex>`.
pub fn hash_file(path: &Path) -> Result<String> {
    let contents = std::fs::read(path)?;
    let digest = Sha256::digest(&contents);
    Ok(format!("sha256:{:x}", digest))
}

/// Compute SHA-256 hash of a directory (all files sorted by relative path).
pub fn hash_dir(dir: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut paths: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    paths.sort();

    for path in paths {
        let contents = std::fs::read(&path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("{}: {e}", path.display()),
            ))
        })?;
        hasher.update(&contents);
    }

    Ok(format!("sha256:{:x}", hasher.finalize()))
}

/// Compute hash for a tracked resource based on its kind.
pub fn hash_resource(kind: TrackedKind, abs_path: &Path) -> Result<String> {
    match kind {
        TrackedKind::Skill => hash_dir(abs_path),
        TrackedKind::Agent
        | TrackedKind::Session
        | TrackedKind::Memory
        | TrackedKind::ProjectConfig
        | TrackedKind::LlmsTxt => hash_file(abs_path),
    }
}

// ---------------------------------------------------------------------------
// Load / save helpers
// ---------------------------------------------------------------------------

/// Load config, migrating from legacy lockfile if this is the first run.
pub fn load_config() -> Result<Config> {
    let cfg_path = config::config_path();
    if cfg_path.exists() {
        return Config::load(&cfg_path);
    }

    // First run — check for legacy .skill-lock.json to migrate
    let legacy_path = config::lock_file_path();
    if legacy_path.exists() {
        let legacy = LegacyLockFile::load(&legacy_path)?;
        let migrated = Config::migrate_from_v3(&legacy);
        migrated.save(&cfg_path)?;

        // Also check for the old agentspec-lock.yml and remove it
        let old_inv = config::inventory_lockfile_path();
        if old_inv.exists() {
            let _ = std::fs::remove_file(&old_inv);
        }

        // Back up legacy lockfile
        let bak = legacy_path.with_extension("json.bak");
        let _ = std::fs::rename(&legacy_path, &bak);

        return Ok(migrated);
    }

    // Also migrate from the intermediate agentspec-lock.yml if it exists
    let old_inv = config::inventory_lockfile_path();
    if old_inv.exists() {
        let data = std::fs::read_to_string(&old_inv)?;
        // The old format had { version, resources } — same fields, just no discovered
        #[derive(Deserialize)]
        struct OldLockfile {
            #[serde(default)]
            resources: Vec<TrackedResource>,
        }
        let old: OldLockfile = serde_yaml::from_str(&data)?;
        let cfg = Config {
            version: 1,
            last_scan: None,
            resources: old.resources,
            discovered: Vec::new(),
        };
        cfg.save(&cfg_path)?;
        let _ = std::fs::remove_file(&old_inv);
        return Ok(cfg);
    }

    Ok(Config::empty())
}

/// Save config to its OS-appropriate path.
pub fn save_config(cfg: &Config) -> Result<()> {
    cfg.save(&config::config_path())
}
