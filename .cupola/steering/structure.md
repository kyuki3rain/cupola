# Project Structure

## Organization Philosophy

Layer separation following Clean Architecture. The `src/` directory is divided into 4 subdirectories — domain / application / adapter / bootstrap — with dependencies constrained to point inward only.

## Directory Patterns

### Domain Layer (`src/domain/`)
**Purpose**: Pure business logic. No framework dependencies
**Contains**: State enum, Event enum, StateMachine (pure functions), Issue entity, Config value object
**Rule**: No I/O. Only derive macros (serde, thiserror) are permitted

### Application Layer (`src/application/`)
**Purpose**: Use cases and port (trait) definitions
**Contains**: PollingUseCase, TransitionUseCase, SessionManager, RetryPolicy, prompt/io helpers
**Subdir**: `port/` — trait definitions for external dependencies (GitHubClient, IssueRepository, ClaudeCodeRunner, etc.)
**Rule**: Depends on domain. Must not import concrete types from adapter

### Adapter Layer (`src/adapter/`)
**Purpose**: External connection implementations
**Subdirs**:
- `inbound/` — CLI (clap)
- `outbound/` — GitHub REST/GraphQL, SQLite, Claude Code, Git worktree

**Rule**: Implements traits from application. May also depend on domain

### Bootstrap Layer (`src/bootstrap/`)
**Purpose**: DI wiring, configuration loading, runtime startup
**Contains**: app.rs (entry point), config_loader.rs, logging.rs
**Rule**: The only place that knows all concrete types across all layers

## Naming Conventions

- **Files**: snake_case (`github_rest_client.rs`)
- **Types**: UpperCamelCase (`GitHubClientImpl`, `PollingUseCase`)
- **Functions**: snake_case (`find_by_issue_number`, `build_session_config`)
- **Constants**: SCREAMING_SNAKE_CASE (`PR_CREATION_SCHEMA`)
- **Port traits**: Named after capabilities (`GitHubClient`, `IssueRepository`, `ClaudeCodeRunner`)
- **Adapters**: Technology name + role (`OctocrabRestClient`, `SqliteIssueRepository`, `GitWorktreeManager`)

## Code Organization Principles

- **One file, one responsibility**: State in state.rs, Event in event.rs — separated
- **mod.rs is re-export only**: Contains no logic
- **Tests within modules**: `#[cfg(test)] mod tests` placed at the end of each file
- **Integration tests in `tests/`**: Define mock adapters and verify use cases end-to-end
