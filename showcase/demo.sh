#!/usr/bin/env bash
# Launch the agentspec TUI against a sanitized demo HOME for showcase capture.
#
# teasr runs this as the terminal scene command (see ../teasr.toml). Captures
# must never show real user data, so everything the TUI renders is seeded
# below from scratch on every run: fake skills, agents, MCP servers, sessions,
# memories, and projects, wired together by running the real agentspec
# pipeline (sync --adopt, mcp add, fleet start) inside the demo HOME.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(dirname "$script_dir")"

# Prefer the freshly built binary so captures match the branch under review.
if [[ -x "$repo_root/target/release/agentspec" ]]; then
  agentspec="$repo_root/target/release/agentspec"
elif [[ -x "$repo_root/target/debug/agentspec" ]]; then
  agentspec="$repo_root/target/debug/agentspec"
else
  agentspec="$(command -v agentspec)"
fi

demo="/tmp/agentspec-demo"
rm -rf "$demo"
mkdir -p "$demo"
export HOME="$demo"
export XDG_CONFIG_HOME="$demo/.config"

# ── Installed tools (detected via their config dirs) ──
for dir in .claude .gemini .cursor .codex .cline .copilot .amp .opencode \
  .openhands .codeium/windsurf .kimi-cli; do
  mkdir -p "$demo/$dir/skills" "$demo/$dir/agents"
done

# ── Skills (adopted + linked by `sync --adopt` below) ──
seed_skill() {
  local name="$1" desc="$2"
  mkdir -p "$demo/.claude/skills/$name"
  cat > "$demo/.claude/skills/$name/SKILL.md" <<EOF
---
name: $name
description: $desc
---

# $name

$desc
EOF
}
seed_skill api-design "Design REST and gRPC APIs with consistent naming, pagination, and error contracts"
seed_skill code-review "Review pull requests for correctness, safety, and style with actionable comments"
seed_skill release-notes "Draft release notes from merged pull requests grouped by change type"
seed_skill sql-tuning "Profile slow queries and suggest indexes, rewrites, and schema changes"
seed_skill test-coverage "Find untested branches and generate table-driven test cases"
seed_skill docs-writer "Write task-focused documentation with runnable examples"

# ── Agents ──
seed_agent() {
  local name="$1" desc="$2" model="$3"
  cat > "$demo/.claude/agents/$name.md" <<EOF
---
name: $name
description: $desc
model: $model
---

You are the $name agent. $desc
EOF
}
seed_agent reviewer "Reviews diffs for bugs and regressions before merge" sonnet
seed_agent architect "Designs module boundaries and phased delivery plans" opus
seed_agent triager "Labels and prioritizes incoming issues" haiku

# ── Projects (Configs tab) + sessions + memories (Claude project memory) ──
seed_project() {
  local name="$1"
  local path="$demo/github/$name"
  mkdir -p "$path/.git"
  printf '# %s\n' "$name" > "$path/README.md"
  printf '# AGENTS.md\n\nBuild: make build. Test: make test.\n' > "$path/AGENTS.md"
  printf '# %s\n\n> Demo project.\n' "$name" > "$path/llms.txt"
  # Claude Code encodes project paths by replacing separators with '-'.
  local encoded="${path//\//-}"
  mkdir -p "$demo/.claude/projects/$encoded/memory"
  echo "$demo/.claude/projects/$encoded"
}

seed_session() {
  local project_dir="$1" id="$2" ts="$3" prompt="$4"
  cat > "$project_dir/$id.jsonl" <<EOF
{"type":"user","timestamp":"$ts","cwd":"/tmp","message":{"content":"$prompt"}}
{"type":"assistant","timestamp":"$ts","message":{"content":[{"type":"text","text":"On it."}]}}
EOF
}

seed_memory() {
  local project_dir="$1" name="$2" type="$3" desc="$4"
  cat > "$project_dir/memory/$name.md" <<EOF
---
name: $name
description: $desc
type: $type
---

$desc
EOF
}

p1=$(seed_project hello-api)
p2=$(seed_project checkout-web)
p3=$(seed_project data-pipeline)

seed_session "$p1" "11111111-aaaa-4aaa-8aaa-000000000001" "2026-07-01T09:12:00Z" "add rate limiting to the public API endpoints"
seed_session "$p1" "11111111-aaaa-4aaa-8aaa-000000000002" "2026-07-03T14:30:00Z" "why is the healthcheck flaky in CI?"
seed_session "$p2" "22222222-bbbb-4bbb-8bbb-000000000001" "2026-07-08T10:05:00Z" "migrate the cart state to server components"
seed_session "$p2" "22222222-bbbb-4bbb-8bbb-000000000002" "2026-07-10T16:45:00Z" "write e2e tests for the discount code flow"
seed_session "$p3" "33333333-cccc-4ccc-8ccc-000000000001" "2026-07-12T08:20:00Z" "backfill the events table without downtime"

seed_memory "$p1" api-versioning project "Public endpoints are versioned under /v1; breaking changes need a new version"
seed_memory "$p1" retry-budget feedback "Prefer bounded retries with jitter; the team rejected unbounded backoff"
seed_memory "$p2" design-tokens project "Use the shared token package for colors and spacing; no hardcoded hex values"
seed_memory "$p3" batch-windows reference "Nightly batch window is 02:00-04:00 UTC; see the runbook for overrides"

# ── MCP servers + fleet + adopt/link everything through the real pipeline ──
"$agentspec" mcp add github --command gh-mcp --args "serve" >/dev/null 2>&1 || true
"$agentspec" mcp add docs --url "https://docs.example.com/mcp" --type http >/dev/null 2>&1 || true
"$agentspec" mcp add postgres --command pg-mcp --args "serve --readonly" --env "PG_DATABASE=demo" >/dev/null 2>&1 || true
"$agentspec" sync --fast --adopt >/dev/null 2>&1 || true
"$agentspec" fleet --backend store start demo >/dev/null 2>&1 || true
"$agentspec" fleet --backend store adopt demo store:demo:agent-1 --name reviewer --tool claude >/dev/null 2>&1 || true
"$agentspec" fleet --backend store adopt demo store:demo:agent-2 --name migrator --tool codex >/dev/null 2>&1 || true
"$agentspec" fleet --backend store mark demo store:demo:agent-2 idle --note "waiting on review" >/dev/null 2>&1 || true

cd "$demo"
exec "$agentspec"
