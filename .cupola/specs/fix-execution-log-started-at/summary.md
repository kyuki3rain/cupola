# fix-execution-log-started-at サマリー

## Feature
`execution_log.started_at` がプロセス終了時刻で記録されるバグを修正し、spawn 時刻を正確に記録する。

## 要件サマリ
- `step7_spawn_processes` で spawn 成功直後に `exec_log_repo.record_start` を呼び `log_id` を取得。
- `log_id` を `SessionManager.register(issue_id, child, log_id)` で `SessionEntry` に保持。
- `step3_process_exit_check` からは `record_start` を削除し `session.log_id` で `record_finish` のみ呼ぶ。
- `record_start` 失敗時は `unwrap_or(0)` でフォールバックし spawn を継続。

## アーキテクチャ決定
- `log_id` 受け渡しは `SessionEntry`（private）と `ExitedSession`（public）に `log_id: i64` を追加する構造で実装。別 `HashMap` 案は状態が分散するため不採用。
- `ExecutionLogRepository` trait シグネチャ・DB schema は変更しない。
- `log_id = 0` は「記録なし」として DB 側で許容される既存動作を踏襲（`record_finish(0, ...)` は UPDATE 0 行）。

## コンポーネント
- `application/session_manager.rs`: `SessionEntry.log_id`, `ExitedSession.log_id`, `register(...)` シグネチャ変更
- `application/polling_use_case.rs`: step7 と step3 の呼び出し順序変更

## 主要インターフェース
- `SessionManager::register(&mut self, issue_id: i64, child: Child, log_id: i64)`
- `struct ExitedSession { issue_id, exit_status, stdout, stderr, log_id }`

## 学び/トレードオフ
- `register()` シグネチャ変更は呼び出し元 1 箇所のみで影響最小。
- SessionManager に log_id を持たせることでセッションライフサイクル情報の一元管理という責務に整合。
- `record_start` の非同期呼び出しは既存 async スコープ内で完結。
