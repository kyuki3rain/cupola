# Project Structure

## Organization Philosophy

Clean Architecture のレイヤー分離。`src/` 配下を domain / application / adapter / bootstrap の 4 ディレクトリに分割し、依存方向を内向きに制約する。

## Directory Patterns

### Domain Layer (`src/domain/`)
**Purpose**: 純粋ビジネスロジック。フレームワーク依存なし
**Contains**: State enum, Event enum, StateMachine（純粋関数）, Issue エンティティ, Config 値オブジェクト
**Rule**: I/O なし。derive マクロ（serde, thiserror）のみ許可

### Application Layer (`src/application/`)
**Purpose**: ユースケースとポート（trait）定義
**Contains**: PollingUseCase, TransitionUseCase, SessionManager, RetryPolicy, prompt/io ヘルパー
**Subdir**: `port/` — 外部依存の trait 定義（GitHubClient, IssueRepository, ClaudeCodeRunner 等）
**Rule**: domain に依存。adapter の具象型を import しない

### Adapter Layer (`src/adapter/`)
**Purpose**: 外部接続の実装
**Subdirs**:
- `inbound/` — CLI（clap）
- `outbound/` — GitHub REST/GraphQL, SQLite, Claude Code, Git worktree

**Rule**: application の trait を実装する。domain にも依存可

### Bootstrap Layer (`src/bootstrap/`)
**Purpose**: DI 配線、設定読み込み、ランタイム起動
**Contains**: app.rs（エントリ）, config_loader.rs, logging.rs
**Rule**: 全レイヤーの具象型を知る唯一の場所

## Naming Conventions

- **ファイル**: snake_case（`github_rest_client.rs`）
- **型**: UpperCamelCase（`GitHubClientImpl`, `PollingUseCase`）
- **関数**: snake_case（`find_by_issue_number`, `build_session_config`）
- **定数**: SCREAMING_SNAKE_CASE（`PR_CREATION_SCHEMA`）
- **ポート trait**: 能力を表す名前（`GitHubClient`, `IssueRepository`, `ClaudeCodeRunner`）
- **アダプター**: 技術名 + 役割（`OctocrabRestClient`, `SqliteIssueRepository`, `GitWorktreeManager`）

## Code Organization Principles

- **1 ファイル 1 責務**: State は state.rs、Event は event.rs に分離
- **mod.rs は re-export のみ**: ロジックを含まない
- **テストはモジュール内**: `#[cfg(test)] mod tests` を各ファイル末尾に配置
- **統合テストは `tests/`**: モック adapter を定義し、ユースケースをエンドツーエンド検証
