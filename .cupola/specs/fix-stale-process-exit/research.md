# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary
- **Feature**: `fix-stale-process-exit`
- **Discovery Scope**: Extension（既存システムへの小規模修正）
- **Key Findings**:
  - `handle_successful_exit` は既に `needs_process()` チェックを内部で持っているが、`handle_failed_exit` には同チェックが存在しない（非対称な実装）
  - stale exit の原因は、stall kill 後に PR マージや Issue クローズが先行した場合に、同ポーリングサイクル後半の step3 が古い終了ステータスを処理すること
  - `needs_process()` は `DesignRunning | DesignFixing | ImplementationRunning | ImplementationFixing` の 4 状態に対して `true` を返す既存の純粋ドメイン関数であり、追加実装不要

## Research Log

### step3_process_exit_check の実装調査

- **Context**: バグ修正箇所を特定するためのコードリーディング
- **Sources Consulted**: `src/application/polling_use_case.rs` L300-L399
- **Findings**:
  - L302: `step3_process_exit_check` は `session_mgr.collect_exited()` から終了セッション一覧を取得
  - L306-309: `issue_repo.find_by_id()` で最新の issue を取得（DB から最新状態を読む）
  - L312-317: `current_pid` をクリアして DB を更新
  - L320-324: 実行ログの開始を記録
  - L326-333: 成功終了時 → `handle_successful_exit` 呼び出し
  - L334-352: 失敗終了時 → `exec_log_repo.record_finish` → `handle_failed_exit` 呼び出し
  - **重要**: L373 で `handle_successful_exit` は `needs_process()` をチェックするが、`handle_failed_exit` には対応するガードなし
- **Implications**: ガードを `step3_process_exit_check` 内のループ冒頭（PID クリア・ログ記録より後、ハンドラ呼び出しより前）に挿入することで両経路を一括でカバーできる

### needs_process() の契約確認

- **Context**: ガードとして使用する関数の動作確認
- **Sources Consulted**: `src/domain/state.rs` L16-24
- **Findings**:
  - `needs_process()` は `DesignRunning | DesignFixing | ImplementationRunning | ImplementationFixing` に対して `true`
  - `DesignReviewWaiting | ImplementationReviewWaiting | Idle | Initialized | Completed | Cancelled` に対して `false`
  - 既存のユニットテストで網羅的に検証済み（L43-58）
- **Implications**: 新たなドメインロジックの追加は不要。既存関数を再利用するだけでよい

### PID クリア・ログ記録のタイミング検討

- **Context**: ガードを挿入する位置の決定（PID クリアやログ記録をスキップすべきかどうか）
- **Findings**:
  - PID は issue が状態遷移済みの場合、既に別経路でクリア済みである可能性が高い
  - ただし、DB 上に残存 PID があれば副作用なくクリアしても問題ない（冪等）
  - 実行ログの「開始」記録は `handle_*` を呼ばない場合には不要で、ノイズになる
- **Implications**: ガードは「issue 取得直後・PID クリアより前」ではなく「ハンドラ呼び出し直前」が最も安全。ただし実行ログ記録（`record_start`）はガードの後に移動するか、または Issue に対して `needs_process()` を確認してから記録すべき。Issue #128 の修正コード例では `continue` をハンドラ呼び出し前に挿入しているため、ログ記録はスキップされない設計となっているが、stale exit に対してログ開始だけ記録されることになる。これは軽微な問題（ログ記録は start のみで finish なし）であり、別 Issue で対応可能。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ガード挿入（ハンドラ呼び出し前） | `needs_process()` が false なら `continue` | 最小変更、既存テスト影響なし | exec_log_repo.record_start が空振りになる（軽微） | Issue #128 提案と一致 |
| handle_failed_exit 内にガード追加 | `handle_failed_exit` 内で `needs_process()` を確認 | 局所変更 | handle_successful_exit との非対称さが残る | 推奨しない |
| SessionManager の改善 | kill 済みセッションをキャンセル済みとしてマーク | 根本対処 | 変更範囲が大きい | 将来改善候補 |

## Design Decisions

### Decision: ガード挿入位置

- **Context**: stale exit をどの段階で無視するか
- **Alternatives Considered**:
  1. `handle_failed_exit` 内に限定的に追加 — 失敗経路のみ対処
  2. `step3_process_exit_check` ループ内、ハンドラ呼び出し直前に追加 — 成功・失敗両方を一括カバー
- **Selected Approach**: Option 2 — ループ内のハンドラ呼び出しより前（exec_log start 記録の後）に挿入
- **Rationale**: `handle_successful_exit` の内部チェック（L373）と対称な保護を外側に持ち上げることで、将来のハンドラ追加時にも漏れが生じない。Issue #128 の提案コードとも一致する。
- **Trade-offs**: `exec_log_repo.record_start` が stale exit で空振りするが、finish 未記録のログが残るだけで機能への実害なし
- **Follow-up**: `record_start` を guard より後に移動することで stale ログも排除できる（別 Issue 候補）

## Risks & Mitigations

- stale exit を無視することで、実際に失敗したプロセスも無視されてしまうリスク → `needs_process()` は DB から最新状態を取得した結果に基づくため、実行中の issue に対して false になることはない
- テストカバレッジ不足 → 競合シナリオ（PR merge 後の stale exit、Issue close 後の stale exit）を対象とするユニットテストを追加することで担保

## References

- `src/application/polling_use_case.rs` — step3_process_exit_check 実装（L300-399）
- `src/domain/state.rs` — needs_process() 定義（L16-24）
