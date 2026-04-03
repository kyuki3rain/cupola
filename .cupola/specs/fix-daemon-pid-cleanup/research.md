# Research & Design Decisions

---
**Purpose**: discovery findings, architectural investigations, and rationale for the technical design.

---

## Summary

- **Feature**: `fix-daemon-pid-cleanup`
- **Discovery Scope**: Simple Addition（既存システムへの1行修正）
- **Key Findings**:
  - `graceful_shutdown()` にはすでに PID ファイル削除コード（行 1064–1069）が存在するが、`polling.run()` がエラーで早期リターンした場合には呼ばれない
  - `pid_file_manager` は `with_pid_file(Box::new(pid_file_manager))` で `PollingUseCase` に move されるため、`start_daemon_child` 内で直接再利用できない
  - `PidFileManager::new()` はパスを受け取るだけで I/O を行わないため、同一パスに対して2つのインスタンスを作成することは安全

## Research Log

### PollingUseCase.run() のエラーパス分析

- **Context**: `graceful_shutdown()` は通常の SIGTERM/SIGINT シャットダウン時にのみ呼ばれる。`run()` が `?` でエラーリターンした場合に PID ファイルが残存するかを確認
- **Sources Consulted**: `src/application/polling_use_case.rs` (行 88–116, 1043–1072)
- **Findings**:
  - `run()` 内の `signal::unix::signal(SignalKind::terminate())?` が失敗すると、`graceful_shutdown()` を経由せずにエラーリターンする
  - ループ内のエラー（`run_cycle()`）はキャッチされるためリターンしない
  - 通常の SIGTERM/SIGINT パスでは `graceful_shutdown()` が確実に呼ばれ、PID ファイルが削除される
- **Implications**: エラーパスのみが問題。`start_daemon_child` に fallback 削除を追加することで保証できる

### pid_file_manager の所有権分析

- **Context**: `start_daemon_child` 内で `pid_file_manager` は `polling.with_pid_file(Box::new(pid_file_manager))` で move される
- **Sources Consulted**: `src/bootstrap/app.rs` (行 506, 543–551)、`src/adapter/outbound/pid_file_manager.rs`
- **Findings**:
  - `PidFileManager::new(path)` はパスを保持するだけ（I/O なし）
  - 同一パスへの2インスタンス作成は安全
  - `delete_pid()` は冪等（ファイル不在時も `Ok(())` を返す）
- **Implications**: `config_dir.join("cupola.pid")` に対して2つ目の `PidFileManager` を作成し、`polling.run().await` 後に `delete_pid()` を呼ぶことで安全に fallback 削除できる

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|-----------|------|------|------------|------|
| A: start_daemon_child に fallback 追加 | run() 後に別インスタンスで delete_pid() | bootstrap 層のみの変更、最小差分 | ファイル2重削除のリスク（冪等なので安全） | 採用 |
| B: polling.run() のエラーパスで delete | run() 内に defer/drop ガード追加 | 漏れが確実に防げる | application 層の変更が必要、複雑度増加 | 今回は不採用 |
| C: with_pid_file を使わず bootstrap で管理 | graceful_shutdown から削除の責務を外す | 責務が明確 | 大規模リファクタが必要 | 今回は不採用 |

## Design Decisions

### Decision: fallback 削除を start_daemon_child に追加（オプション A 採用）

- **Context**: エラーパスで PID ファイルが残存する可能性がある
- **Alternatives Considered**:
  1. オプション A — bootstrap 層のみの1行追加
  2. オプション B — application 層（polling_use_case.rs）の変更
- **Selected Approach**: `start_daemon_child` 内で `polling.run().await` の結果を保存後、別インスタンスの `PidFileManager` で `delete_pid()` を呼び、エラーは無視してポーリング結果を返す
- **Rationale**: Clean Architecture の原則に従い変更範囲を bootstrap 層に限定。アプリケーション層への影響ゼロ
- **Trade-offs**: 2つの `PidFileManager` インスタンスが同一パスを指すが、`delete_pid()` の冪等性で安全
- **Follow-up**: 正常終了・エラー終了両方のパスで PID ファイルが削除されることをテストで確認する

## Risks & Mitigations

- PID ファイルが既に削除済みの状態で fallback 削除を呼ぶ — `delete_pid()` は冪等（`Ok(())` を返す）なので安全
- `polling.run()` の結果を fallback 削除後に返すため、エラーが正しく伝播されるか — `let result = polling.run().await; let _ = pid_file_manager_cleanup.delete_pid(); result` のパターンで保証

## References

- `src/bootstrap/app.rs:483-554` — `start_daemon_child` 関数
- `src/application/polling_use_case.rs:88-116` — `run()` メソッド
- `src/application/polling_use_case.rs:1043-1072` — `graceful_shutdown()` メソッド
- `src/adapter/outbound/pid_file_manager.rs` — `PidFileManager` 実装
- `src/application/port/pid_file.rs` — `PidFilePort` トレイト定義
