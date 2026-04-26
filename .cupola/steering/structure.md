# Project Structure

## Organization Philosophy

Layer separation following Clean Architecture. The `src/` directory is divided into 4 subdirectories — domain / application / adapter / bootstrap — with dependencies constrained to point inward only.

## Directory Patterns

### Domain Layer (`src/domain/`)
**Purpose**: Pure business logic. No framework dependencies
**Contains**:
- Decision core: `decide()` pure function + `Decision` (next_state + metadata_updates + effects) + `Effect` enum + `WorldSnapshot` (GitHub issue/PR/CI observations)
- Entities & VOs: Issue, Config, State, Phase, TaskWeight, ModelConfig, ProcessRun, FixingProblemKind, ExecutionLog, MetadataUpdates, ShutdownMode, ClaudeSettings / ClaudeCodeEnvConfig
- Security: AuthorAssociation / TrustedAssociations (label actor trust value objects)

**Rule**: No I/O. Only derive macros (serde, thiserror) are permitted. `decide()` takes `(prev: &Issue, snap: &WorldSnapshot, cfg: &Config)` and returns a `Decision` — all side effects are emitted as `Effect` values, never performed.

### Application Layer (`src/application/`)
**Purpose**: Use cases and port (trait) definitions
**Contains**: PollingUseCase, StartUseCase, StopUseCase, InitUseCase, DoctorUseCase, CleanupUseCase, CompressUseCase, LogsUseCase, SessionManager, RetryPolicy, InitTaskManager, TemplateManager, AssociationGuard (label actor trust check), InitAgent enum, prompt/io helpers
**Subdir `polling/`**: 5-stage pipeline (`collect` → `resolve` → decide → `persist` → `execute`) — observation, snapshot build, call into `domain::decide`, persist state/metadata, then run effects
**Subdir `port/`**: trait definitions for outbound dependencies (GitHubClient, IssueRepository, ClaudeCodeRunner, ExecutionLogRepository, ProcessRunRepository, FileGenerator, GitWorktree, PidFilePort, CommandRunner, ConfigLoader, DbInitializer, SignalPort)
**Rule**: Depends on domain. Must not import concrete types from adapter

### Adapter Layer (`src/adapter/`)
**Purpose**: External connection implementations
**Subdirs**:
- `inbound/` — CLI (clap)
- `outbound/` — GitHub REST/GraphQL, SQLite (Issue + ExecutionLog, plus `SqliteConnection` wrapper), Claude Code (`ClaudeCodeProcess`), Git worktree, PID file manager, init file generator, process command runner, `NixSignalSender` (SIGTERM/SIGKILL via `nix` crate)

**Rule**: Implements traits from application. May also depend on domain

### Bootstrap Layer (`src/bootstrap/`)
**Purpose**: DI wiring, configuration loading, runtime startup
**Contains**: app.rs (entry point + daemon launch), config_loader.rs, toml_config_loader.rs, logging.rs
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
