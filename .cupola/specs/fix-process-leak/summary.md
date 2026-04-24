# fix-process-leak サマリー

## Feature
`polling_use_case.rs` の 2 つのプロセス管理バグ修正: `graceful_shutdown` のループ終了判定と `recover_on_startup` での孤児プロセス kill。

## 要件サマリ
1. `graceful_shutdown` のループ継続条件を `collect_exited().is_empty()` から `session_mgr.count() == 0` に変更。デッドライン (10秒) 到達時は `kill_all()` + 警告ログ。
2. `recover_on_startup` で `current_pid` が Some の場合、プロセス生存を確認し生存時は SIGKILL で終了してから DB クリア。死亡済みなら既存どおりクリアのみ。
3. 両修正にユニットテストを追加、`cargo test` / `cargo clippy -D warnings` を通す。

## アーキテクチャ決定
- `is_process_alive(pid: u32) -> bool` を `polling_use_case.rs` のモジュールレベル自由関数として実装。既存 `stop_use_case.rs` の nix 使用パターンに準拠。
- `ProcessChecker` port trait 案はオーバーエンジニアリングのため不採用。`SessionManager` にメソッド追加する案は責務拡大のため不採用。
- 生存確認はシグナル 0 (`kill(pid, None)`)。`Err(ESRCH)` は不在、`EPERM` 等は保守的に true 返却（SIGKILL 試行）。
- PID 範囲バリデーション (`1..=i32::MAX as u32`) は呼び出し元で実施。
- application 層が nix を直接参照するが既存パターンと一貫しており許容。

## コンポーネント
- `PollingUseCase::graceful_shutdown`（ループ条件変更）
- `PollingUseCase::recover_on_startup`（孤児 kill 追加）
- `is_process_alive` 自由関数（新規、`#[cfg(unix)]`）

## 主要インターフェース
- `fn is_process_alive(pid: u32) -> bool`
- `SessionManager::count() -> usize`（既存）を新たにループ判定に利用

## 学び/トレードオフ
- `collect_exited()` は「終了済みのみ」を返すため SIGKILL 直後は空になりうる。真の完了指標は `count() == 0`。
- PID 再利用リスクは低いが info ログに PID と issue_id を記録し監査可能に。
- SIGKILL の ESRCH はシグナル 0 と SIGKILL の間の自然終了競合を吸収。
