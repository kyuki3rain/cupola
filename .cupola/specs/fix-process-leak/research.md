# Research & Design Decisions

---
**Purpose**: fix-process-leak の調査記録と設計意思決定の根拠。

---

## Summary
- **Feature**: `fix-process-leak`
- **Discovery Scope**: Extension（既存 `polling_use_case.rs` の2箇所のバグ修正）
- **Key Findings**:
  - `collect_exited()` は終了済みセッションのみを返すため、kill 直後でセッションがまだ実行中だと空リストを返す。`count() == 0` がループ終了の正しい判定条件。
  - `nix` クレート（v0.29, process+signal feature）は既に Cargo.toml に含まれており、`stop_use_case.rs` と `pid_file_manager.rs` で `nix::sys::signal::kill` + `nix::unistd::Pid` のパターンが確立済み。
  - プロセス生存確認にはシグナル 0 送信（`kill(pid, None)`）が標準的な Unix 手法。`Ok(())` なら生存、`Err(ESRCH)` なら不在。

## Research Log

### graceful_shutdown のループ判定バグ

- **Context**: `collect_exited()` が `Vec<ExitedSession>` を返す。SIGKILL 直後はプロセスが OS に回収される前のため、`try_wait()` が `Ok(None)` を返し、`collect_exited()` が空リストを返す可能性がある。
- **Sources Consulted**: `src/application/session_manager.rs` (lines 74-126), `src/application/polling_use_case.rs` (lines 1216-1227)
- **Findings**:
  - `collect_exited()` は `try_wait()` で終了を確認したセッションのみ HashMap から除去して返す
  - `count()` は `self.sessions.len()` を返す（line 128-130）
  - `collect_exited()` が空でも sessions に残っているケースが発生しうる
- **Implications**: ループ終了条件を `exited.is_empty()` から `self.session_mgr.count() == 0` に変更する必要がある

### recover_on_startup での孤児プロセス確認

- **Context**: `current_pid` が Some の場合に実プロセスの生存を確認せずクリアしている
- **Sources Consulted**: `src/application/polling_use_case.rs` (lines 1191-1206), `src/application/stop_use_case.rs`, `src/adapter/outbound/pid_file_manager.rs`, `Cargo.toml`
- **Findings**:
  - `nix = { version = "0.29", features = ["process", "signal"] }` が既存依存に存在
  - `stop_use_case.rs` で `nix::sys::signal::{Signal, kill}` と `nix::unistd::Pid` を使うパターンが確立済み
  - `kill(Pid::from_raw(pid), None)` （シグナル 0）はプロセス存在確認の標準手法
- **Implications**: 同パターンで `is_process_alive(pid: u32) -> bool` 関数を実装可能。新規クレート不要。

## Architecture Pattern Evaluation

| オプション | 説明 | 長所 | リスク |
|-----------|------|------|--------|
| A: PollingUseCase 内の自由関数 | `is_process_alive` をモジュールレベル関数として polling_use_case.rs 内に実装 | 変更が局所的、ポート新設不要 | application 層が OS 依存（nix）を直接持つ |
| B: ProcessChecker ポート（trait） | application 層に `ProcessChecker` trait を定義し adapter で nix 実装 | Clean Architecture 準拠 | 単純 bug fix に対してオーバーエンジニアリング |
| C: SessionManager にメソッド追加 | `recover_on_startup` 専用で SessionManager に孤児チェックメソッドを追加 | SessionManager にプロセス管理を集約 | SessionManager の責務が広がる |

**選択: A（PollingUseCase 内の自由関数）**
- 既存の `stop_use_case.rs` も application 層で nix を直接使用しており、プロジェクトのコーディングスタイルと一致する
- バグ修正スコープを最小化し、テスト容易性も維持できる

## Design Decisions

### Decision: `graceful_shutdown` ループ条件の変更

- **Context**: `collect_exited()` が空を返しても、まだ終了処理中のセッションが存在しうる
- **Alternatives Considered**:
  1. `exited.is_empty()` のまま（現状）- SIGKILL 直後のレースコンディションが残る
  2. `self.session_mgr.count() == 0` に変更 - 全セッション回収済みを正確に判定
- **Selected Approach**: `count() == 0` による判定
- **Rationale**: `collect_exited()` は副作用として終了セッションを HashMap から除去する。`count() == 0` は全セッションが除去済みであることを表す唯一の正確な条件。
- **Trade-offs**: `collect_exited()` の戻り値を無視するため、呼び出しの目的が "プロセス回収（副作用）" であることをコメントで明示する必要がある
- **Follow-up**: デッドライン到達時の強制 kill 処理は変更不要

### Decision: `is_process_alive` の実装場所

- **Context**: `recover_on_startup` は application 層の `PollingUseCase` 内に存在
- **Selected Approach**: `polling_use_case.rs` 内のモジュールレベル自由関数として実装
- **Rationale**: 既存の `stop_use_case.rs` の nix 使用パターンに倣い、テスト・コードレビューの一貫性を保つ
- **Trade-offs**: テスト時に関数のモック置き換えが不要（SIGKILL の副作用はインテグレーションテストでカバー）

## Risks & Mitigations

- **シグナル 0 の競合状態**: PID 確認後と SIGKILL 送信の間にプロセスが終了する可能性 → SIGKILL 失敗（`ESRCH`）は無視し、警告ログのみとする
- **PID 再利用**: 同一 PID が別プロセスに再割り当てされる可能性 → DB の `current_pid` はデーモンセッション固有の設計であり、運用上のリスクは低い
- **Windows 非対応**: `nix` クレートは Unix 専用 → Cargo.toml の `[target.'cfg(unix)'.dependencies]` ゲートを維持する

## References

- nix crate docs: https://docs.rs/nix/0.29/ — signal handling と Pid の API リファレンス
- POSIX kill(2): signal 0 によるプロセス存在確認の標準的用法
