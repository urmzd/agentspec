---
name: release-helper
description: >
  Cuts releases and validates changelogs across the workspace.
  Use when preparing or auditing a release.
license: Apache-2.0
allowed-tools: Bash Read Grep
compatibility: Requires git 2.40+
user-invocable: true
metadata:
  author: urmzd
  version: "1.2.0"
customVendorField: 1
---

# Release Helper

## Instructions

Run the release pipeline:

1. Validate the changelog.
2. Tag the release.
