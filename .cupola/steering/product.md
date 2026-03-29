# Product Overview

A locally-resident agent that uses GitHub Issues and PRs as the sole interface, driving Claude Code + cc-sdd to automate design and implementation. Humans only create Issues, assign labels, and review PRs — everything from design document generation to implementation, review response, and completion cleanup is automated.

## Core Capabilities

- **Issue Detection -> Automated Design Generation**: Detects Issues with the `agent:ready` label via polling and automatically generates requirements / design / tasks using cc-sdd
- **PR-Based Review Flow**: Automatically creates design PRs and implementation PRs, and automates fixes, replies, and resolution on review threads
- **State Machine Driven**: Manages the entire workflow through a 10-state state machine, supporting idempotent re-execution and graceful shutdown
- **Separation of Responsibilities**: Cupola handles all GitHub API operations; Claude Code only performs git operations (commit/push)

## Target Use Cases

- Automating iterative feature development for individuals and small teams
- End-to-end automation from design to implementation, starting from an Issue
- Automated fix and reply cycle in response to review comments

## Value Proposition

Leverages GitHub's existing workflow (Issues + PRs + review) as-is, automating design and implementation without any dedicated UI. Human review approval serves as the sole gate, achieving both quality assurance and automation.
