# cupola-agent

## Feature
Cupola の本体である Rust 製自動化エージェント。GitHub Issues/PR をインターフェースとして、Claude Code を子プロセスとして spawn し、設計 → 実装 → レビュー対応までを 10 状態のステートマシンで自律的に駆動する。1 サイクル 7 ステップの polling ループが中核。greenfield で新規構築。

## 要件サマリ
- **ステートマシン**: 10 状態の `State` enum と `Event` enum を定義し、`StateMachine::transition` を純粋関数として実装。全 (State, Event) 組み合わせを網羅。
- **状態遷移**: idle → initialized → design_running → design_review_waiting → implementation_running → implementation_review_waiting → completed / cancelled を基本フロー。design_fixing / implementation_fixing を review 対応サブ状態として持つ。
- **Polling 7 ステップ**: (1) Issue 検知・close 確認、(2) Initialized 復旧再試行、(3) プロセス終了回収・後処理、(4) PR 監視（merge 優先 → review thread）、(5) stall 検知 + kill、(6) イベントバッチ適用、(7) needs_process な Issue の spawn。
- **Claude Code 起動**: `-p <prompt> --output-format json --json-schema <schema> --dangerously-skip-permissions`。PR 作成用と fixing 用の 2 種類の output-schema を使い分ける。
- **入出力ファイル**: `.cupola/inputs/issue.md`（design_running）、`.cupola/inputs/review_threads.json`（fixing）を worktree に書き出し。
- **GitHub API**: REST（octocrab）と GraphQL（reqwest 直接）を使い分けて単一 `GitHubClient` trait に統合。review thread の resolve は GraphQL 必須。
- **永続化**: SQLite（rusqlite）に `issues` / `execution_log` テーブル。WAL + busy_timeout。
- **設定**: `cupola.toml` + CLI フラグ（CLI > TOML > default）。`clap` で `run` / `init` / `status` サブコマンド。
- **Graceful shutdown**: SIGINT → ループ停止 → 全プロセスに SIGTERM → 10 秒後 SIGKILL。
- **再起動復旧**: 起動時に非 terminal Issue の `current_pid` を NULL 化し、次サイクルで spawn を再開。terminal レコード上の idle 再起動は `reset_for_restart` で上書き。

## アーキテクチャ決定
- **Clean Architecture 4 レイヤー採用**: domain / application / adapter / bootstrap。代替として Hexagonal（命名違い）、Simple Layered（境界曖昧）を検討。ステートマシン駆動のドメインロジックを純粋に保ち、外部依存（GitHub / SQLite / Claude Code / Git）をアダプタに隔離。CLAUDE.md の clean-architecture ルールにも合致。
- **プロセス管理**: `tokio::process` ではなく `std::process::Command` + `try_wait()` + stdout/stderr 読み取りスレッド方式。polling ループの同期サイクルと自然に統合でき、SessionManager の設計が単純化。stdout/stderr は別スレッドで蓄積してパイプバッファ 64KB 枯渇を防止。
- **GitHubClient 単一 trait**: REST/GraphQL を分離せず統一 trait へ。アプリケーション層は内部の API 切替えを関知しない。trait が肥大化したら将来 sub-trait 分割を検討。
- **イベントバッチ適用**: polling サイクル内で全イベントを収集してから TransitionUseCase にバッチ適用。即時適用だと同一 Issue 複数イベント時の優先制御（IssueClosed 最優先）が困難。Initialized 復旧のみ即時適用の例外。1 サイクル分の遅延は許容。
- **rusqlite + tokio 統合**: `Arc<Mutex<Connection>>` + 全 DB 操作を `tokio::task::spawn_blocking` でラップ。`.await` 中に Mutex ロックを保持しないことを保証する設計規律。
- **冪等性**: PR 作成前に DB → `find_pr_by_branches` の二段チェック。worktree 作成・削除、ブランチ作成・削除はいずれも対象不在でスキップ。
- **retry 管理**: 正常遷移時は `retry_count` を 0 にリセット。`RetryPolicy` が `max_retries` との比較で Retry / Exhausted を判定し、Exhausted で Cancelled に遷移。
- **Cleanup 非ブロッキング**: completed / cancelled の cleanup は各操作を個別 try し、失敗してもログ記録のみで状態遷移をブロックしない。
- **野良プロセス対策の割り切り**: 再起動時の PID 再利用リスクを避けるため `current_pid` による kill はせず、stall timeout による自然回収に委ねる。`.git/index.lock` は spawn 前に削除して競合回避（TOCTOU リスクは残存）。
- **GraphQL pagination**: `first: 100` + `pageInfo.hasNextPage/endCursor` で reviewThreads と comments の両方を cursor-based で対応。

## コンポーネント
### domain
- `State` / `Event` enum、`StateMachine::transition` 純粋関数
- `Issue` エンティティ（github_issue_number, state, design_pr_number, impl_pr_number, worktree_path, retry_count, current_pid, error_message, timestamps）
- `Config` 値オブジェクト、`ExecutionLog`

### application
- `PollingUseCase`（7 ステップメインループ）
- `TransitionUseCase`（イベント適用 + 副作用実行）
- `SessionManager`（HashMap で `Child` 管理、register / collect_exited / kill / kill_all / find_stalled）
- `RetryPolicy`、`build_session_config`（state 別プロンプトと OutputSchemaKind を返す）
- プロンプト定数（design_running / implementation_running / fixing）と output-schema JSON 定数
- 入力ファイル writer（`issue.md` / `review_threads.json`）と出力 JSON parser（PR 用 / fixing 用）

### adapter
- `GitHubClientImpl`: REST（octocrab）+ GraphQL（reqwest）を内部合成
- `SqliteIssueRepository` / `SqliteExecutionLogRepository`
- `ClaudeCodeProcess`（`std::process::Command` で `-p` / `--output-format json` / `--json-schema` / `--dangerously-skip-permissions` を付与）
- `GitWorktreeManager`（worktree add / remove、branch create / delete、pull、push、冪等）

### bootstrap
- `main.rs` → `bootstrap/app.rs` で cupola.toml パース、Config 構築、CLI オーバーライド、tracing 初期化
- DI で具象アダプタを PollingUseCase に注入
- `run` / `init` / `status` コマンド実装

## 主要インターフェース
- `GitHubClient` trait: `list_ready_issues`, `get_issue`, `is_issue_open`, `find_pr_by_branches`, `is_pr_merged`, `create_pr`, `comment_on_issue`, `close_issue`, `list_unresolved_threads`, `reply_to_thread`, `resolve_thread`
- `IssueRepository` trait: `find_by_id`, `find_by_issue_number`, `find_active`, `find_needing_process`, `save`, `update_state`, `update`, `reset_for_restart`
- `ExecutionLogRepository` trait: `record_start`, `record_finish`, `find_by_issue`
- `StateMachine::transition(state, event) -> Result<State, TransitionError>`
- Claude Code 起動: `-p <prompt> --output-format json --dangerously-skip-permissions [--json-schema <schema>]`
- output-schema（PR 用）: `pr_title`, `pr_body`
- output-schema（fixing 用）: `threads[].{thread_id, response, resolved}`
- 入力ファイル: `.cupola/inputs/issue.md`（`# Issue #N: title / ## Labels / ## Body`）、`.cupola/inputs/review_threads.json`（thread_id, path, line, comments[]）

## 学び / トレードオフ
- ステートマシンを純粋関数として分離することで、adapter モック注入による統合テストが容易になった。`idle → completed` / `idle → cancelled` の主要パスをエンドツーエンドで検証可能。
- review thread の resolve が REST 非対応のため GraphQL が必須。REST/GraphQL を分ける trait は冗長になるため単一 trait に統合したのは正解だが、GraphQL クエリの型安全性は `serde_json::Value` + 手動マッピングで妥協している。
- `std::process` 採用により SessionManager のコードは単純になったが、stdout/stderr 読み取りスレッドの JoinHandle 管理が独立した責務となった。非同期化は将来検討余地。
- イベントのバッチ適用方式は「同一 Issue に複数イベント発生時の順序制御」を保証する一方、1 サイクル分の遅延を生む。polling_interval の調整で吸収。
- 野良プロセスの自然回収を stall timeout に委ねる割り切りは運用上の受容判断。PID 再利用リスクを完全に排除するには Unix の procfs や名前空間が必要で、本スコープでは過剰と判断。
- `current_pid` と `retry_count` の永続化により、restart 後も前回状態を復旧できる。ただし SessionManager は空から開始し、次サイクルで spawn し直す設計。これにより restart 時の整合性が簡単に確保できる。
- GraphQL pagination の実装コストが高かったため、reviewThreads と comments の両方で first=100 + cursor 方式に統一。
- `--dangerously-skip-permissions` は自動化に必須だが、セキュリティリスクを認識した上での許容。
- tracing + tracing-appender による構造化ログ（issue_number, state を span フィールド）は debug 時の情報粒度に大きく貢献。
