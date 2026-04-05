# Resources: Open Standards for AI Agent Configuration

A reference of open standards, specifications, and conventions that define how AI agents discover context, capabilities, and project instructions. These standards inform how `agentspec` discovers and verifies resources.

---

## Agent Skills (agentskills.io)

- **Website**: https://agentskills.io
- **Spec**: https://agentskills.io/specification
- **GitHub**: https://github.com/agentskills/agentskills
- **Example skills**: https://github.com/anthropics/skills

An open format for extending AI agents with specialized knowledge and workflows. A skill is a directory containing a `SKILL.md` file with YAML frontmatter and Markdown instructions.

### Directory Structure

```
skill-name/
├── SKILL.md          # Required: metadata + instructions
├── scripts/          # Optional: executable code
├── references/       # Optional: documentation
├── assets/           # Optional: templates, resources
```

### SKILL.md Frontmatter

| Field           | Required | Description                                                              |
|-----------------|----------|--------------------------------------------------------------------------|
| `name`          | Yes      | 1-64 chars, lowercase alphanumeric + hyphens, must match directory name  |
| `description`   | Yes      | 1-1024 chars, describes what the skill does and when to use it           |
| `license`       | No       | License name or reference to bundled license file                        |
| `compatibility` | No       | 1-500 chars, environment requirements                                    |
| `metadata`      | No       | Arbitrary key-value mapping                                              |
| `allowed-tools` | No       | Space-delimited list of pre-approved tools (experimental)                |

### Discovery Paths

| Scope   | Path                                | Purpose                       |
|---------|-------------------------------------|-------------------------------|
| Project | `<project>/.<client>/skills/`       | Client-specific skills        |
| Project | `<project>/.agents/skills/`         | Cross-client interop          |
| User    | `~/.<client>/skills/`               | Client-specific user skills   |
| User    | `~/.agents/skills/`                 | Cross-client user skills      |

### Progressive Disclosure

1. **Catalog** (~50-100 tokens/skill): Name + description loaded at startup
2. **Instructions** (<5000 tokens): Full SKILL.md body on activation
3. **Resources** (varies): Scripts/references/assets loaded on demand

### Supported Agents

Claude Code, Cursor, VS Code (Copilot), GitHub Copilot, Gemini CLI, OpenCode, OpenHands, Goose, Roo Code, Amp, Junie (JetBrains), Kiro, OpenAI Codex, Letta, and many more.

---

## llms.txt (llmstxt.org)

- **Website**: https://llmstxt.org
- **Spec**: https://llmstxt.org/llms.txt

A standard for adding a Markdown file at a website's root to provide LLM-friendly content, enabling language models to understand websites efficiently without parsing complex HTML.

### File Location

`/llms.txt` at the root path of a website (or optionally in subpaths).

### Format (Markdown)

```markdown
# Project Name                          (required: H1 heading)

> Brief summary with key info            (optional: blockquote)

Additional details as markdown body      (optional: paragraphs, lists, etc.)

## Section Name                          (optional: H2-delimited file lists)
- [Link title](https://url): Description

## Optional                              (special: skippable for shorter contexts)
- [Secondary resource](https://url): Description
```

### Variants

| File            | Purpose                                    |
|-----------------|--------------------------------------------|
| `/llms.txt`     | Curated overview with key links            |
| `/llms-full.txt`| Complete detailed documentation for LLMs   |

---

## AGENTS.md (agents.md)

- **Website**: https://agents.md
- **Stewarded by**: Agentic AI Foundation (Linux Foundation)

An open format to guide AI coding agents through project contexts and workflows. Described as "a README for agents" — it separates agent-focused technical guidance from human-oriented documentation.

### Design Principles

1. **Clarity for agents**: A clear, predictable place for instructions
2. **Human-focused READMEs**: Keeps README.md concise for humans
3. **Complementary guidance**: Precise agent-focused guidance alongside existing docs

### Format

Standard Markdown with no mandatory fields or sections. Common sections include:

- Project overview
- Build and test commands
- Code style guidelines
- Testing instructions
- Security considerations
- Development environment tips
- PR/commit message guidelines
- Deployment steps

### Hierarchical Resolution

For monorepos, nested `AGENTS.md` files take precedence based on file proximity — the closest file to the working directory wins.

### Adoption

60k+ open-source projects. Supported by Cursor, Zed, VS Code, GitHub Copilot, and others.

---

## Model Context Protocol (MCP)

- **Website**: https://modelcontextprotocol.io
- **Spec**: https://modelcontextprotocol.io/docs/learn/architecture

An open-source protocol for connecting AI applications to external systems — data sources, tools, and workflows. Analogous to USB-C for AI: a standardized way to connect.

### Key Concepts

- **Servers**: Expose data sources, tools, and prompts
- **Clients**: AI applications that connect to MCP servers
- **Tools**: Executable capabilities servers expose to agents
- **Resources**: Data and content servers make available
- **Prompts**: Reusable prompt templates

### Configuration Files

| Agent       | File              | Location               |
|-------------|-------------------|------------------------|
| Claude Code | `.mcp.json`       | Project root           |
| Claude Code | `~/.claude.json`  | User-level (mcpServers key) |
| Claude Code | `managed-mcp.json`| System dirs (macOS: /Library/Application Support/ClaudeCode/) |
| Cursor      | `.cursor/mcp.json`| Project root           |
| Gemini CLI  | `settings.json`   | `~/.gemini/`           |
| VS Code     | `.vscode/mcp.json`| Project root           |

### .mcp.json Format

```json
{
  "mcpServers": {
    "server-name": {
      "command": "/path/to/server",
      "args": ["--flag", "value"],
      "env": {
        "API_KEY": "${API_KEY}"
      }
    }
  }
}
```

### Server Types

| Type             | Transport          | Use case                    |
|------------------|--------------------|-----------------------------|
| stdio            | stdin/stdout       | Local process servers       |
| sse              | Server-Sent Events | Remote streaming servers    |
| streamable-http  | HTTP POST + SSE    | Remote servers (recommended)|

### Server Definition Fields

| Field     | Required | Description                                      |
|-----------|----------|--------------------------------------------------|
| `command` | Yes*     | Executable path for stdio servers                |
| `args`    | No       | Command arguments array                          |
| `env`     | No       | Environment variables (supports `${VAR}` expansion)|
| `url`     | Yes*     | URL for HTTP/SSE transport servers               |
| `type`    | No       | Transport type (stdio, sse, streamable-http)     |

\* One of `command` or `url` required depending on transport type.

### Scopes

| Scope   | Storage                        | Shared? |
|---------|--------------------------------|---------|
| Local   | `~/.claude.json` (per-project) | No      |
| Project | `.mcp.json` in project root    | Yes     |
| User    | `~/.claude.json` (mcpServers)  | No      |
| Managed | `managed-mcp.json` (system)    | IT-deployed |

### MCP Registry

Anthropic hosts an MCP server registry at `https://api.anthropic.com/mcp-registry/v0/servers` for discovering available servers.

---

## Claude Code Configuration

- **Docs**: https://code.claude.com/docs/en/settings
- **CLI Reference**: https://code.claude.com/docs/en/cli-reference

### CLAUDE.md (Memory Files)

Instructions and context that Claude loads at startup. Markdown format, no schema.

| Location                          | Scope                                   |
|-----------------------------------|-----------------------------------------|
| `~/.claude/CLAUDE.md`             | User — applies to all projects          |
| `CLAUDE.md` or `.claude/CLAUDE.md`| Project — shared with team via git      |
| `CLAUDE.local.md`                 | Local — personal, gitignored            |

### settings.json

| Scope   | Location                           | Shared? |
|---------|------------------------------------|---------|
| Managed | System-level `managed-settings.json` | IT-deployed |
| User    | `~/.claude/settings.json`          | No      |
| Project | `.claude/settings.json`            | Yes     |
| Local   | `.claude/settings.local.json`      | No      |

Precedence (highest to lowest): Managed > CLI args > Local > Project > User.

### Key Settings

| Key                    | Description                                           |
|------------------------|-------------------------------------------------------|
| `permissions.allow`    | Tools/patterns allowed without prompting               |
| `permissions.deny`     | Tools/patterns always denied                           |
| `hooks`                | Lifecycle event commands                               |
| `env`                  | Environment variables for every session                |
| `model`                | Default model override                                 |
| `agent`                | Run main thread as named subagent                      |
| `attribution`          | Git commit/PR attribution customization                |
| `enabledPlugins`       | Plugin enable/disable map                              |
| `sandbox`              | Filesystem/network sandbox configuration               |

### Subagents

Markdown files with YAML frontmatter stored in:
- `~/.claude/agents/` (user-level)
- `.claude/agents/` (project-level)

### Skills

Discovered from `.claude/skills/` directories following the Agent Skills specification.

---

## Gemini CLI Configuration

- **GitHub**: https://github.com/google-gemini/gemini-cli

### GEMINI.md

Project-specific context file (analogous to CLAUDE.md). Markdown, no schema.

### settings.json

Located at `~/.gemini/settings.json`. Supports MCP server configuration.

### Other Files

| File              | Purpose                        |
|-------------------|--------------------------------|
| `.geminiignore`   | File exclusion patterns         |
| `GEMINI.md`       | Project context for Gemini CLI  |

---

## Cursor Configuration

- **Docs**: https://cursor.com/docs

### .cursor/rules

Project-level AI configuration directory. Supports rules files that guide Cursor's AI behavior.

### .cursorrules (Legacy)

Single file at project root with AI instructions (deprecated in favor of `.cursor/rules/`).

---

## Cross-Standard Comparison

| Standard      | File(s)                  | Format       | Discovery         | Purpose                  |
|---------------|--------------------------|--------------|-------------------|--------------------------|
| Agent Skills  | `SKILL.md`               | YAML + MD    | Directory scan    | Agent capabilities       |
| llms.txt      | `/llms.txt`              | Markdown     | HTTP root path    | LLM-friendly site info   |
| AGENTS.md     | `AGENTS.md`              | Markdown     | Filesystem walk   | Agent project guidance   |
| MCP           | `.mcp.json`              | JSON         | Config files      | Tool/data integration    |
| CLAUDE.md     | `CLAUDE.md`              | Markdown     | Known paths       | Project instructions     |
| GEMINI.md     | `GEMINI.md`              | Markdown     | Known paths       | Project instructions     |
| .cursorrules  | `.cursor/rules/`         | Markdown     | Known paths       | Project instructions     |

---

## Relevant Links

### Specifications
- https://agentskills.io/specification
- https://llmstxt.org
- https://agents.md
- https://modelcontextprotocol.io

### Agent Tools
- https://code.claude.com/docs/en/settings
- https://code.claude.com/docs/en/cli-reference
- https://github.com/google-gemini/gemini-cli
- https://cursor.com/docs

### Repositories
- https://github.com/agentskills/agentskills
- https://github.com/anthropics/skills
- https://github.com/modelcontextprotocol

### Community
- https://discord.gg/MKPE9g8aUy (Agent Skills Discord)
