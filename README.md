# Cupola

[![CI](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg)](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/kyuki3rain/cupola/blob/main/LICENSE)

[日本語](./README.ja.md)

Issue-driven local agent control plane for spec-driven development.

## Table of Contents

- [Project Overview](#project-overview)
- [Prerequisites](#prerequisites)
- [Installation & Setup](#installation--setup)
- [Usage](#usage)
- [CLI Command Reference](#cli-command-reference)
- [Configuration Reference](#configuration-reference)
- [Security: trusted_associations](#security-trusted_associations)
- [Fork Workflow for External Contributors](#fork-workflow-for-external-contributors)
- [Architecture Overview](#architecture-overview)
- [Limitations](#limitations)
- [License](#license)

## Project Overview

Cupola is a locally-resident agent that uses GitHub Issues and PRs as its sole interface, driving Claude Code to automate design and implementation. Humans only create Issues, assign labels, and review PRs. Cupola handles everything from design document generation to implementation, review response, and completion cleanup. By leveraging GitHub's existing workflow (Issues + PRs + reviews), Cupola achieves both quality assurance and automation without any dedicated UI.

**Key Features:**

- **Automated design generation**: Detects GitHub Issues and generates requirements, design, and tasks through bundled Cupola skills
- **Automatic PR creation**: Creates design PRs and implementation PRs without manual intervention
- **Review thread handling**: Automatically fixes, replies, and resolves review threads on PRs
- **CI failure auto-fix**: Detects CI (GitHub Actions, etc.) failures and automatically attempts to fix them
- **Conflict auto-fix**: Detects merge conflicts and automatically attempts to resolve them
- **Model override via Issue labels**: Attach labels like `model:opus` to an Issue to override the Claude model used for that Issue
- **Concurrent session limit**: Use `max_concurrent_sessions` to cap the number of simultaneously running agent sessions
- **Environment & config check**: Run `cupola doctor` to validate Cupola configuration and GitHub integration (config file, git/gh setup, labels, steering, DB)

## Prerequisites

> **Platform**: Unix (macOS / Linux) only. Windows is not supported due to the dependency on the `nix` crate (`cfg(unix)`).

| Tool | Purpose | Notes |
|------|---------|-------|
| Rust stable | Build | Managed via devbox |
| Claude Code CLI | AI code generation | Provided by Anthropic |
| gh CLI | GitHub API operations | GitHub official |
| Git | Version control | — |
| devbox | Development environment management | Nix-based |

Cupola bootstraps its own rules, templates, and Claude Code skills into the target repository via `cupola init`. Design and implementation then run through bundled `/cupola:*` commands rather than an external skill dependency.

When using devbox, run `devbox shell` at the repository root to set up all required tools (Rust, etc.) at once.

## Installation & Setup

1. Clone the repository

   ```bash
   git clone https://github.com/kyuki3rain/cupola.git
   cd cupola
   ```

2. Enter the development environment (when using devbox)

   ```bash
   devbox shell
   ```

   If not using devbox, install Rust stable manually.

3. Build and install

   ```bash
   cargo install --path .
   ```

   > `cargo install` places the `cupola` binary in `~/.cargo/bin/`. Make sure `~/.cargo/bin` is in your PATH.

4. Bootstrap Cupola into the repository

   ```bash
   cupola init
   ```

   This creates `.cupola/cupola.toml`, installs bundled Cupola assets, updates `.gitignore`, initializes the SQLite DB, and attempts to generate initial steering with Claude Code.

5. Fill in `.cupola/cupola.toml`

6. Create the `agent:ready` label in your GitHub repository

   ```bash
   gh label create "agent:ready" --description "Cupola automation trigger"
   ```

7. Start polling

   ```bash
   cupola start
   ```

## Usage

Workflow from Issue creation to merge:

1. **[Human]** Create a GitHub Issue and describe the requirements
2. **[Human]** Add the `agent:ready` label to the Issue — this triggers Cupola
3. **[Cupola]** Detects the Issue and auto-generates design documents (requirements / design / tasks) using bundled Cupola skills
4. **[Cupola]** Creates a design PR
5. **[Human]** Reviews and approves the design PR
6. **[Cupola]** Auto-generates the implementation based on the tasks
7. **[Cupola]** Creates an implementation PR
8. **[Human]** Reviews the implementation PR, approves, and merges
9. **[Cupola]** Executes cleanup (label removal, etc.)

The two-stage review flow (design PR and implementation PR) ensures quality with human review approval as the sole gate.

## CLI Command Reference

### `cupola start`

Starts the polling loop and monitors Issues with the `agent:ready` label.

| Option | Description | Default |
|--------|-------------|---------|
| `--polling-interval-secs <seconds>` | Override polling interval (seconds) | Value from `cupola.toml` |
| `--log-level <level>` | Override log level (trace / debug / info / warn / error) | Value from `cupola.toml` |
| `--config <path>` | Configuration file path | `.cupola/cupola.toml` |
| `-d`, `--daemon` | Run as a background daemon | false |

```bash
# Start with default settings
cupola start

# Start as a background daemon with custom polling interval
cupola start --daemon --polling-interval-secs 30
```

### `cupola stop`

Stops a running background daemon.

| Option | Description | Default |
|--------|-------------|---------|
| `--config <path>` | Configuration file path | `.cupola/cupola.toml` |

```bash
cupola stop
```

### `cupola doctor`

Checks that all required tools and configuration are in place.

| Option | Description | Default |
|--------|-------------|---------|
| `--config <path>` | Configuration file path | `.cupola/cupola.toml` |

```bash
cupola doctor
```

### `cupola init`

Bootstraps Cupola into the current repository for the target agent runtime.

```bash
cupola init
```

### `cupola status`

Lists the processing status of all Issues.

```bash
cupola status
```

### `cupola --version` / `-V`

Displays the installed version.

```bash
cupola --version
```

## Configuration Reference

The configuration file is located at `.cupola/cupola.toml`.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `owner` | String | — (required) | GitHub repository owner |
| `repo` | String | — (required) | GitHub repository name |
| `default_branch` | String | — (required) | Default branch name |
| `language` | String | `"ja"` | Language for generated documents |
| `polling_interval_secs` | u64 | `60` | Polling interval (seconds) |
| `max_retries` | u32 | `3` | Maximum retry count |
| `stall_timeout_secs` | u64 | `1800` | Stall detection timeout (seconds) |
| `max_concurrent_sessions` | u32 | `3` | Maximum number of concurrent Cupola sessions |
| `model` | String | `"sonnet"` | Default Claude model for agent sessions |
| `trusted_associations` | Array of String or `["all"]` | `["OWNER", "MEMBER", "COLLABORATOR"]` | Author associations trusted to trigger the agent |
| `[log] level` | String | `"info"` | Log level |
| `[log] dir` | String | — (optional) | Log output directory |

Full configuration example:

```toml
owner = "your-github-username"
repo = "your-repo-name"
default_branch = "main"
language = "ja"
polling_interval_secs = 60
max_retries = 3
stall_timeout_secs = 1800
max_concurrent_sessions = 4  # default: 3
model = "sonnet"

# Security: only owners, members, and collaborators can trigger the agent (default)
trusted_associations = ["OWNER", "MEMBER", "COLLABORATOR"]

# For private repositories: trust all users (disables association check)
# trusted_associations = ["all"]

[log]
level = "info"
dir = ".cupola/logs"
```

## Security: trusted_associations

Cupola passes GitHub Issue content and PR review comments to Claude Code. In public repositories,
this creates a **prompt injection** risk: a malicious user could craft Issue/PR content to
manipulate the agent's behavior.

The `trusted_associations` setting mitigates this by only allowing users with a trusted
GitHub `author_association` to:

- Apply the `agent:ready` label (which triggers the agent)
- Have their review comments passed to the agent

**Valid values**: `OWNER`, `MEMBER`, `COLLABORATOR`, `CONTRIBUTOR`, `FIRST_TIMER`,
`FIRST_TIME_CONTRIBUTOR`, `NONE`

**Special value**: `"all"` — skips association checks entirely. **Only use this on private
repositories** where all users are trusted.

See [SECURITY.md](SECURITY.md) for a detailed explanation of the security model.

## Fork Workflow for External Contributors

If you are an external contributor (not an `OWNER`, `MEMBER`, or `COLLABORATOR`) who wants to
use Cupola on your own changes before submitting a PR upstream, you can use the fork workflow:

1. **Fork the repository** on GitHub.

2. **Enable Cupola on your fork**: Clone your fork and set up `cupola.toml` in your fork.
   Since you own the fork, your `author_association` will be `OWNER`.

3. **Develop using Cupola on your fork**: Create Issues on your fork, apply the `agent:ready`
   label, and let Cupola generate designs and implementation on your fork.

4. **Submit a PR upstream**: Once the implementation is ready on your fork's branch, open a
   PR from your fork's branch to the upstream repository.

5. **Upstream reviewers** will review the PR as usual.

This workflow gives external contributors full Cupola automation power on their own fork
without needing special permissions on the upstream repository.

## Architecture Overview

Cupola adopts Clean Architecture (4 layers). Dependencies point inward only.

| Layer | Directory | Responsibility |
|-------|-----------|----------------|
| domain | `src/domain/` | Pure business logic. State, Event, StateMachine, Issue, Config. No I/O dependencies |
| application | `src/application/` | Use cases and port (trait) definitions. External dependencies abstracted via traits |
| adapter | `src/adapter/` | External connection implementations. inbound (CLI) / outbound (GitHub, SQLite, Claude Code, Git) |
| bootstrap | `src/bootstrap/` | DI wiring, configuration loading, runtime startup |

Dependency direction: `domain` ← `application` ← `adapter` ← `bootstrap` (inward only)

```
src/
├── main.rs
├── lib.rs
├── domain/
│   ├── check_result.rs
│   ├── config.rs
│   ├── event.rs
│   ├── execution_log.rs
│   ├── fixing_problem_kind.rs
│   ├── issue.rs
│   ├── state.rs
│   └── state_machine.rs
├── application/
│   ├── doctor_use_case.rs
│   ├── error.rs
│   ├── init_use_case.rs
│   ├── io.rs
│   ├── polling_use_case.rs
│   ├── prompt.rs
│   ├── retry_policy.rs
│   ├── session_manager.rs
│   ├── stop_use_case.rs
│   ├── transition_use_case.rs
│   └── port/
│       ├── claude_code_runner.rs
│       ├── command_runner.rs
│       ├── config_loader.rs
│       ├── execution_log_repository.rs
│       ├── git_worktree.rs
│       ├── github_client.rs
│       ├── issue_repository.rs
│       └── pid_file.rs
├── adapter/
│   ├── inbound/
│   │   └── cli.rs
│   └── outbound/
│       ├── claude_code_process.rs
│       ├── git_worktree_manager.rs
│       ├── github_client_impl.rs
│       ├── github_graphql_client.rs
│       ├── github_rest_client.rs
│       ├── init_file_generator.rs
│       ├── pid_file_manager.rs
│       ├── process_command_runner.rs
│       ├── sqlite_connection.rs
│       ├── sqlite_execution_log_repository.rs
│       └── sqlite_issue_repository.rs
└── bootstrap/
    ├── app.rs
    ├── config_loader.rs
    ├── logging.rs
    └── toml_config_loader.rs
```

## Limitations

- **Review comment scope**: Only PR review threads (`review_thread`) are supported. Top-level PR review comments (PR-level comments without a thread) are not handled.
- **Quality check commands**: Quality check commands that Cupola runs must be defined in the target repository's `AGENTS.md` or `CLAUDE.md`.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.
