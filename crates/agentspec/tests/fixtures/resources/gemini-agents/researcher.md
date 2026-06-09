---
name: researcher
description: Performs background research with citations.
tools:
  - google_web_search
  - web_fetch
model: gemini-2.5-pro
temperature: 0.4
max_turns: 8
timeout_mins: 15
kind: local
mcpServers:
  context7:
    command: context7-mcp
customGeminiField: experimental
---

Research the topic and cite every source you use.
