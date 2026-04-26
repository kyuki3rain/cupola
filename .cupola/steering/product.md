# Product Overview

A locally-resident agent that uses GitHub Issues and PRs as the sole interface, driving Claude Code to automate design and implementation. Humans only create Issues, assign labels, and review PRs — everything from design document generation to implementation, review response, and completion cleanup is automated.

Design and implementation are executed through bundled `/cupola:*` skills (Claude Code slash commands) that Cupola bootstraps into the target repository via `cupola init`. These skills are original implementations inspired by Kiro's spec-driven development approach, with no external skill dependencies.

## Core Capabilities

- **Issue Detection → Automated Design Generation**: Detects Issues with the `agent:ready` label via polling and automatically generates requirements / design / tasks via the bundled `/cupola:spec-design` skill
- **PR-Based Review Flow**: Automatically creates design PRs and implementation PRs, and automates fixes, replies, and resolution on review threads
- **State Machine Driven**: Manages the entire workflow through a multi-state state machine, supporting idempotent re-execution and graceful shutdown
- **Separation of Responsibilities**: Cupola handles all GitHub API operations (push, PR creation, review replies, thread resolution); Claude Code (via bundled skills) handles design generation, implementation, and local git operations (add, commit) only

## Bundled Skills (`/cupola:*`)

Deployed to `.claude/commands/cupola/` by `cupola init`:

- **`/cupola:spec-init`**: Initialize a new spec directory (`spec.json` + `requirements.md` scaffolding)
- **`/cupola:spec-design`**: One-pass generation of requirements (EARS format), research, design, and tasks from an Issue description
- **`/cupola:spec-impl`**: TDD-based implementation of spec tasks
- **`/cupola:spec-compress`**: Archive and summarize completed specs
- **`/cupola:fix`**: Address review comments, CI failures, and merge conflicts on a Cupola-managed PR
- **`/cupola:steering`**: Maintain `.cupola/steering/` as persistent project memory

## Target Use Cases

- Automating iterative feature development for individuals and small teams
- End-to-end automation from design to implementation, starting from an Issue
- Automated fix and reply cycle in response to review comments

## Value Proposition

Leverages GitHub's existing workflow (Issues + PRs + review) as-is, automating design and implementation without any dedicated UI or external skill dependency. Human review approval serves as the sole gate, achieving both quality assurance and automation.
