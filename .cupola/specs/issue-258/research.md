# Research & Design Decisions

---
**Purpose**: 設計判断の根拠と調査記録。

---

## Summary

- **Feature**: issue-258 — StopUseCase シグナル失敗時の PID ファイルリーク修正
- **Discovery Scope**: Extension（既存 application layer use case への局所的バグ修正）
- **Key Findings**:
  - 修正対象は `src/application/stop_use_case.rs` の 2 箇所のみ
  - `is_process_alive()` および `delete_pid()` は既に同 use case 内で使用されており、クリーンアップロジックを追加するための API は整っている
  - 新しい依存関係・外部ライブラリは一切不要
  - 既存テスト (T-6.SP.*) はすべて維持可能

## Research Log

### 現行コードのバグ箇所の特定

- **Context**: `send_sigterm()` / `send_sigkill()` は `Result<(), StopError>` を返す。現行コードは `?` でエラーを即座に伝播しているため、その後の `delete_pid()` 呼び出しに到達しない
- **Findings**:
  - Line 42: `self.signal.send_sigterm(pid)?;` — 失敗時に `delete_pid()` 未呼び出しで即リターン
  - Line 59: `self.signal.send_sigkill(pid)?;` — 同様
  - `is_process_alive()` はすでに line 51, 64 で呼ばれており再利用可能
  - `delete_pid()` はすでに line 52, 70 で呼ばれており再利用可能
- **Implications**: `?` を `if let Err(e) = ...` パターンに置き換え、エラーパスで `is_process_alive` + `delete_pid` を呼ぶだけで修正可能

### MockPidFile の delete_pid 呼び出し検証方法

- **Context**: テストで `delete_pid()` が実際に呼ばれたかを検証する必要がある
- **Findings**: 既存の `MockPidFile` は `delete_pid()` が呼ばれても何もしない。`Arc<Mutex<bool>>` の `deleted` フラグを追加するパターンが `alive`、`sigkill_sent` と同形で自然に適合する
- **Implications**: テストには `deleted: Arc<Mutex<bool>>` フィールドを `MockPidFile` に追加し、`delete_pid()` で `true` に設定する変形 mock を使う

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: 明示的エラーパス処理 | `if let Err(e) = send_sigterm(pid)` ブロック内でクリーンアップ | シンプル・追加型不要・レビューしやすい | 将来エラーパスが増えた場合に繰り返しが生じる | 今回は 2 箇所のみ → 採用 |
| B: RAII PidCleanupGuard | `Drop` で `is_process_alive` チェック → `delete_pid` を呼ぶガード構造体 | 将来のコード変更に対してロバスト | 新型追加・Drop の非同期制約に注意が必要 | エラーパスが多い場合に有効。今回は過剰 |
| C: Result コンビネータ | `.or_else(|e| ...)` チェーン | 関数型スタイル | 可読性が下がる場合がある | 採用しない |

## Design Decisions

### Decision: 明示的エラーパス処理（Option A）の採用

- **Context**: シグナル送信の失敗パスは現在 2 箇所のみ（SIGTERM, SIGKILL）
- **Alternatives Considered**:
  1. Option A — `if let Err(e)` ブロックで明示的クリーンアップ
  2. Option B — RAII `PidCleanupGuard` 構造体
- **Selected Approach**: Option A
- **Rationale**: 修正範囲が単一ファイル・2 箇所のみであり、新型不要・可読性最大
- **Trade-offs**: コードの繰り返しが 2 箇所に生じるが、ロジックが明快で許容範囲
- **Follow-up**: 将来エラーパスが増える場合は RAII ガードへのリファクタリングを検討

### Decision: delete_pid エラーを swallow してシグナルエラーを優先

- **Context**: クリーンアップパスで `delete_pid()` が失敗した場合、呼び出し元には「シグナル送信エラー」と「PID 削除エラー」の 2 つが発生する
- **Selected Approach**: `delete_pid()` のエラーは `tracing::warn!` でログ記録し、元のシグナルエラーを返す
- **Rationale**: 呼び出し元が扱うべき根本原因はシグナル送信失敗であり、副次的な cleanup エラーは観測性を維持しつつ上位に隠蔽する
- **Trade-offs**: PID 削除失敗が隠れるが、ログで観測可能

## Risks & Mitigations

- SIGKILL 送信失敗後 `is_process_alive()` の競合 — OS がプロセスを reap するまでに時間がかかることがある。既存コードの 100ms 待機パスとは別の失敗パスであり、ここでは即座に確認する。実環境でのタイミング問題は既存の動作と同等
- `delete_pid()` の二重呼び出し — クリーンアップパスと正常パスで両方呼ばれないよう、エラーパスでは `return Err(e)` で確実に早期リターンする設計を維持する

## References

- `src/application/stop_use_case.rs` — 修正対象
- `src/application/port/pid_file.rs` — `PidFilePort` トレイト定義
- `src/application/port/signal.rs` — `SignalPort` トレイト定義
- Issue #212 — 本修正の分割元
