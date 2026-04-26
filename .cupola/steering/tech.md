# Technology Stack

## Architecture

Clean Architecture (4 layers). Dependencies point inward only.

- **domain**: Pure business logic (State, Event, StateMachine, Issue, Config). No I/O dependencies
- **application**: Use cases + port definitions (traits). External dependencies abstracted via traits
- **adapter**: External connection implementations (GitHub API, SQLite, Claude Code, Git)
- **bootstrap**: DI wiring, configuration loading, runtime startup

## Core Technologies

- **Language**: Rust (Edition 2024)
- **Runtime**: tokio (async runtime, signal handling)
- **Storage**: SQLite (rusqlite, WAL mode)
- **GitHub API**: octocrab (REST) + reqwest (direct GraphQL POST)

## Key Libraries

| Purpose | Crate | Pattern |
|---------|-------|---------|
| CLI | clap (derive) | Subcommands: start / stop / init / status / doctor / cleanup / logs |
| GitHub REST | octocrab | Personal token authentication |
| GitHub GraphQL | reqwest + serde_json | Direct POST, Value parsing |
| DB | rusqlite | Arc<Mutex<Connection>>, spawn_blocking |
| Logging | tracing + tracing-appender | Structured logging, date-based file output |
| Error | thiserror (domain/app) + anyhow (adapter/bootstrap) | Layer-specific usage |
| i18n | rust-i18n | GitHub comments and prompt localization |
| Signal | nix | POSIX signal sending (SIGTERM/SIGKILL) via `NixSignalSender` implementing `SignalPort` |

## Development Standards

### Type Safety
- Static guarantees through Rust's type system
- All ports defined as traits, implementations isolated in adapters
- State/Event use enums with exhaustive pattern matching

### Code Quality
- `cargo clippy -- -D warnings` (all warnings treated as errors)
- `cargo fmt` (unified formatting via rustfmt)
- `[lints.clippy] all = "warn"`, `expect_used = "deny"` in Cargo.toml

### Testing
- Unit tests: `#[cfg(test)]` blocks within each module
- Integration tests: `tests/` directory, mock adapter injection
- SQLite tests use in-memory DB

## Development Environment

### Required Tools
- Rust stable (via devbox, rustup)
- devbox (Nix-based development environment management)

### Common Commands

All commands use `devbox run` — cargo is managed by devbox and may not be in PATH outside of `devbox shell`.

```bash
devbox run build        # Build
devbox run test         # Run all tests
devbox run check        # Type check (no codegen)
devbox run clippy       # Static analysis
devbox run fmt          # Format code
devbox run fmt-check    # Format check (CI)

# CLI subcommands — devbox run cupola <subcommand>
devbox run cupola start        # Start polling loop (foreground)
devbox run cupola start -d     # Start as daemon (background)
devbox run cupola stop         # Stop daemon (SIGTERM → SIGKILL)
devbox run cupola init         # Initialize SQLite schema + steering files
devbox run cupola status       # List Issue states
devbox run cupola doctor       # Validate config, GitHub, git
devbox run cupola cleanup      # Remove worktrees for Cancelled issues
devbox run cupola logs         # View log files
```

## Key Technical Decisions

- **std::process vs tokio::process**: Adopted std::process + try_wait() for natural integration with the polling loop. stdout/stderr accumulated in separate threads
- **Single GitHubClient trait**: Hides REST/GraphQL distinction from the application layer. Composed using the facade pattern
- **Decide/Effect decision core**: `domain::decide` is a pure function `(Issue, WorldSnapshot, Config) → Decision`. A `Decision` carries the next State, metadata updates, and a priority-ordered `Vec<Effect>`. Side effects are *data*; the `execute` stage interprets them. This keeps state transitions unit-testable without mocks.
- **5-stage polling pipeline** (`application/polling/`): `collect` fetches GitHub signals → `resolve` builds a `WorldSnapshot` per issue → `decide` (domain) produces a `Decision` → `persist` writes state/metadata changes → `execute` runs effects (spawn, comment, cleanup, close). Effects are sorted by priority so transition effects run before persistent ones.
- **Daemon mode**: Re-exec strategy (`--daemon-child`) to avoid fork() inside tokio runtime. PID file based lifecycle management
- **Session management**: HashMap of issue_id → running process, with concurrent session limiting and stall detection
- **Doctor sections**: `DoctorUseCase` outputs results split into `StartReadiness` (prerequisites for starting the daemon) and `OperationalReadiness` (runtime health checks), each check carrying an optional `remediation` hint
- **Association guard**: `check_label_actor` in `application/association_guard.rs` verifies the `agent:ready` label actor's GitHub association against `TrustedAssociations` config. `All` skips the API call; `Specific(...)` fetches Timeline + Permission APIs and removes the label + posts a comment on rejection
