# Cupola

[日本語](./README.ja.md)

A locally-resident agent that automates from design to implementation, starting from GitHub Issues.

## Table of Contents

- [Project Overview](#project-overview)
- [Prerequisites](#prerequisites)
- [Installation & Setup](#installation--setup)
- [Usage](#usage)
- [CLI Command Reference](#cli-command-reference)
- [Configuration Reference](#configuration-reference)
- [Architecture Overview](#architecture-overview)
- [License](#license)

## Project Overview

Cupola is a locally-resident agent that uses GitHub Issues and PRs as its sole interface, driving Claude Code + cc-sdd to automate design and implementation. Humans only create Issues, assign labels, and review PRs — Cupola handles everything from design document generation to implementation, review response, and completion cleanup. By leveraging GitHub's existing workflow (Issues + PRs + reviews), Cupola achieves both quality assurance and automation without any dedicated UI.

## Prerequisites

| Tool | Purpose | Notes |
|------|---------|-------|
| Rust stable | Build | Managed via devbox |
| Claude Code CLI | AI code generation | Provided by Anthropic |
| gh CLI | GitHub API operations | GitHub official |
| Git | Version control | — |
| devbox | Development environment management | Nix-based |

**cc-sdd (spec-driven development)** is a specification-driven development methodology that progressively advances through requirements definition, design, task decomposition, and implementation. Cupola internally drives cc-sdd to automatically generate requirements, design, and tasks from Issue content before proceeding with implementation.

When using devbox, run `devbox shell` at the repository root to set up all required tools (Rust, etc.) at once.

## Installation & Setup

1. Clone the repository

   ```bash
   git clone https://github.com/<owner>/<repo>.git
   cd <repo>
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

4. Create `.cupola/cupola.toml`

   ```toml
   owner = "your-github-username"
   repo = "your-repo-name"
   default_branch = "main"
   ```

   See [Configuration Reference](#configuration-reference) for details on all settings.

5. Initialize the SQLite schema

   ```bash
   cupola init
   ```

6. Create the `agent:ready` label in your GitHub repository

   ```bash
   gh label create "agent:ready" --description "Cupola automation trigger"
   ```

7. Start polling

   ```bash
   cupola run
   ```

## Usage

Workflow from Issue creation to merge:

1. **[Human]** Create a GitHub Issue and describe the requirements
2. **[Human]** Add the `agent:ready` label to the Issue — this triggers Cupola
3. **[Cupola]** Detects the Issue and auto-generates design documents (requirements / design / tasks) using cc-sdd
4. **[Cupola]** Creates a design PR
5. **[Human]** Reviews and approves the design PR
6. **[Cupola]** Auto-generates the implementation based on the tasks
7. **[Cupola]** Creates an implementation PR
8. **[Human]** Reviews the implementation PR, approves, and merges
9. **[Cupola]** Executes cleanup (label removal, etc.)

The two-stage review flow (design PR and implementation PR) ensures quality with human review approval as the sole gate.

## CLI Command Reference

### `cupola run`

Starts the polling loop and monitors Issues with the `agent:ready` label.

| Option | Description | Default |
|--------|-------------|---------|
| `--polling-interval-secs <seconds>` | Override polling interval (seconds) | Value from `cupola.toml` |
| `--log-level <level>` | Override log level (trace / debug / info / warn / error) | Value from `cupola.toml` |
| `--config <path>` | Configuration file path | `.cupola/cupola.toml` |

```bash
# Start with default settings
cupola run

# Start with custom polling interval and log level
cupola run --polling-interval-secs 30 --log-level debug
```

### `cupola init`

Initializes the SQLite schema. Run once during initial setup.

```bash
cupola init
```

### `cupola status`

Lists the processing status of all Issues.

```bash
cupola status
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

[log]
level = "info"
dir = ".cupola/logs"
```

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
│   ├── config.rs
│   ├── event.rs
│   ├── execution_log.rs
│   ├── issue.rs
│   ├── state.rs
│   └── state_machine.rs
├── application/
│   ├── error.rs
│   ├── io.rs
│   ├── polling_use_case.rs
│   ├── prompt.rs
│   ├── retry_policy.rs
│   ├── session_manager.rs
│   ├── transition_use_case.rs
│   └── port/
│       ├── claude_code_runner.rs
│       ├── execution_log_repository.rs
│       ├── git_worktree.rs
│       ├── github_client.rs
│       └── issue_repository.rs
├── adapter/
│   ├── inbound/
│   │   └── cli.rs
│   └── outbound/
│       ├── claude_code_process.rs
│       ├── git_worktree_manager.rs
│       ├── github_client_impl.rs
│       ├── github_graphql_client.rs
│       ├── github_rest_client.rs
│       ├── sqlite_connection.rs
│       ├── sqlite_execution_log_repository.rs
│       └── sqlite_issue_repository.rs
└── bootstrap/
    ├── app.rs
    ├── config_loader.rs
    └── logging.rs
```

## License

> License is TBD. A link will be added here once the LICENSE file is created.
