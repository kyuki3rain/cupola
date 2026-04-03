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
| CLI | clap (derive) | Subcommands: start / stop / init / status / doctor |
| GitHub REST | octocrab | Personal token authentication |
| GitHub GraphQL | reqwest + serde_json | Direct POST, Value parsing |
| DB | rusqlite | Arc<Mutex<Connection>>, spawn_blocking |
| Logging | tracing + tracing-appender | Structured logging, date-based file output |
| Error | thiserror (domain/app) + anyhow (adapter/bootstrap) | Layer-specific usage |

## Development Standards

### Type Safety
- Static guarantees through Rust's type system
- All ports defined as traits, implementations isolated in adapters
- State/Event use enums with exhaustive pattern matching

### Code Quality
- `cargo clippy -- -D warnings` (all warnings treated as errors)
- `cargo fmt` (unified formatting via rustfmt)
- `[lints.clippy] all = "warn"` in Cargo.toml

### Testing
- Unit tests: `#[cfg(test)]` blocks within each module
- Integration tests: `tests/` directory, mock adapter injection
- SQLite tests use in-memory DB

## Development Environment

### Required Tools
- Rust stable (via devbox, rustup)
- devbox (Nix-based development environment management)

### Common Commands
```bash
cargo build          # Build
cargo test           # Run all tests
cargo clippy         # Static analysis
cargo fmt --check    # Format check
cargo run -- start   # Start polling loop
cargo run -- init    # Initialize SQLite schema
cargo run -- status  # List Issue states
```

## Key Technical Decisions

- **std::process vs tokio::process**: Adopted std::process + try_wait() for natural integration with the polling loop. stdout/stderr accumulated in separate threads
- **Single GitHubClient trait**: Hides REST/GraphQL distinction from the application layer. Composed using the facade pattern
- **Event batch application**: Collects all events within a polling cycle and applies them in batch, prioritizing IssueClosed
