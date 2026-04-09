//! Static registry of all known project file specs across editors/tools.
//! Adding support for a new file type or tool = one entry in the table.

use crate::inventory::TrackedKind;

/// A single project file specification.
pub struct ProjectFileSpec {
    /// Resource kind for this file type
    pub kind: TrackedKind,
    /// Editor/tool slug (matches tools::CodingTool::slug), or "open-spec" for vendor-neutral
    pub editor: &'static str,
    /// Canonical filename used in the index
    pub filename: &'static str,
    /// Path relative to the project root
    pub project_path: &'static str,
    /// Path relative to $HOME (for global files), if any
    pub global_path: Option<&'static str>,
    /// Whether the entry is a directory (e.g. `.cursor/rules/`)
    pub is_directory: bool,
}

/// All known project file specs — open-spec configs, instruction files, and llms.txt.
pub static PROJECT_FILES: &[ProjectFileSpec] = &[
    // ── Open spec ──
    ProjectFileSpec {
        kind: TrackedKind::ProjectConfig,
        editor: "open-spec",
        filename: "AGENTS.md",
        project_path: "AGENTS.md",
        global_path: None,
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::LlmsTxt,
        editor: "open-spec",
        filename: "llms.txt",
        project_path: "llms.txt",
        global_path: None,
        is_directory: false,
    },
    // ── Instruction files (editor-specific) ──
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "claude-code",
        filename: "CLAUDE.md",
        project_path: "CLAUDE.md",
        global_path: Some(".claude/CLAUDE.md"),
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "gemini-cli",
        filename: "GEMINI.md",
        project_path: "GEMINI.md",
        global_path: Some(".gemini/GEMINI.md"),
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "github-copilot",
        filename: "copilot-instructions.md",
        project_path: ".github/copilot-instructions.md",
        global_path: None,
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "codex",
        filename: "codex-instructions.md",
        project_path: "codex.md",
        global_path: Some(".codex/instructions.md"),
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "cursor",
        filename: "cursorrules",
        project_path: ".cursorrules",
        global_path: None,
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "cursor",
        filename: "cursor-rules",
        project_path: ".cursor/rules",
        global_path: None,
        is_directory: true,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "cline",
        filename: "clinerules",
        project_path: ".clinerules",
        global_path: None,
        is_directory: false,
    },
    ProjectFileSpec {
        kind: TrackedKind::InstructionFile,
        editor: "windsurf",
        filename: "windsurfrules",
        project_path: ".windsurfrules",
        global_path: None,
        is_directory: false,
    },
];

/// Find all project file specs that exist in a given project root.
pub fn find_in_project(
    project_root: &std::path::Path,
) -> Vec<(&'static ProjectFileSpec, std::path::PathBuf)> {
    let mut found = Vec::new();
    for spec in PROJECT_FILES {
        let path = project_root.join(spec.project_path);
        if path.exists() {
            found.push((spec, path));
        }
    }
    found
}

/// Find all global project files (instruction files with global_path).
pub fn find_global() -> Vec<(&'static ProjectFileSpec, std::path::PathBuf)> {
    let home = crate::config::home_dir();
    let mut found = Vec::new();
    for spec in PROJECT_FILES {
        if let Some(global_path) = spec.global_path {
            let path = home.join(global_path);
            if path.exists() {
                found.push((spec, path));
            }
        }
    }
    found
}
