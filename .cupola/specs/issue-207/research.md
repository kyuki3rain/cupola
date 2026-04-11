# Research & Design Decisions

---
**Purpose**: issue-207 の設計調査と意思決定の記録。

---

## Summary

- **Feature**: Running フェーズ GitHub API エラー局所化（issue-207）
- **Discovery Scope**: Extension（既存システムへの局所的な修正）
- **Key Findings**:
  - Fixing フェーズ（行 316–326）に `if let Err(e)` を用いたベストエフォートパターンが既存実装として存在し、そのままモデルとして流用できる
  - `ProcessRunRepository::mark_failed()` は `run_id: i64` と `error_message: Option<String>` を受け取り、`state=failed / error_message / finished_at` を DB に記録する
  - ループの呼び出し側（`process_exited_session`）はエラーが返ると即座に中断する設計であるため、ローカルで `Ok(())` を返すことが必須

## Research Log

### Running フェーズの現状コード分析

- **Context**: `resolve.rs` 行 289–309 で `find_pr_by_branches().await?` / `create_pr().await?` がエラーを直接伝播している
- **Sources Consulted**: `src/application/polling/resolve.rs`（直接コード読取り）
- **Findings**:
  - `?` 演算子により GitHub API エラーが `process_exited_session` 呼び出し元に伝播し、ループが中断される
  - `run_id > 0` ガードが存在し、run_id が 0 の場合は DB 更新をスキップする既存ロジックがある
  - ブランチ名（`head_branch`, `base_branch`）は当該スコープ内に変数として存在する
- **Implications**: `?` を除去して `if let Err(e)` パターンに置き換えることで影響範囲を最小化できる

### Fixing フェーズのベストエフォートパターン

- **Context**: 行 316–326 に `reply_to_thread` / `resolve_thread` に対してエラーを握り潰しログのみ出力するパターンがある
- **Findings**:
  - `if let Err(e) = github.call().await { tracing::warn!(...) }` という一貫した記述スタイル
  - `mark_failed` の呼び出しはなく、ベストエフォートのまま `mark_succeeded` に進む
- **Implications**: Running フェーズは Fixing フェーズと異なり、PR 作成失敗は後処理失敗に分類されるため `mark_failed` の呼び出しが必要

### ProcessRun 状態遷移ルール

- **Context**: `docs/architecture/polling-loop.md`（イシュー本文記載）の状態遷移表
- **Findings**:
  - `Running 系 + 成功、後処理失敗` → `state=failed, error_message セット, pid クリア, finished_at セット`
  - この遷移は `mark_failed(run_id, Some(e.to_string()))` で実現される
- **Implications**: GitHub API 失敗は「後処理失敗」に分類され、`mark_failed` を呼び出す必要がある

### `mark_failed` シグネチャと伝播方針

- **Context**: `src/application/port/process_run_repository.rs` 行 25–28
- **Findings**:
  - `async fn mark_failed(&self, run_id: i64, error_message: Option<String>) -> Result<()>`
  - `mark_failed` 自体のエラーは DB 層の障害であり、ループ継続不可とみなして `?` で伝播する（イシュー本文に明記）
- **Implications**: GitHub API エラーは握り込み、`mark_failed` エラーは伝播するという非対称な設計が意図されている

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| `if let Err(e)` + `mark_failed` | エラーキャプチャ後に ProcessRun を failed に更新し `Ok(())` を返す | Fixing フェーズとの一貫性、最小変更 | なし | **採用** |
| `?` のまま呼び出し元で retry | 呼び出し元でリトライロジックを追加 | — | ループ中断問題が解消されない | 不採用 |
| Result を Vec に集約して後処理 | 全セッション処理後にまとめてエラー報告 | バッチ処理に向いている | 設計変更が大きい | 不採用 |

## Design Decisions

### Decision: GitHub API エラーを `mark_failed` + `Ok(())` で局所化する

- **Context**: ループ継続性と ProcessRun 状態整合性の両立が必要
- **Alternatives Considered**:
  1. `?` 伝播のまま呼び出し元でハンドリング — ループ中断の根本原因を解消できない
  2. Fixing フェーズと同様に `mark_failed` なしでログのみ — ProcessRun 状態が `succeeded` になり状態機械が誤動作する
- **Selected Approach**: `if let Err(e)` で GitHub API エラーをキャプチャし、`mark_failed(run_id, Some(e.to_string())).await?` を呼び出した後に `return Ok(())` する
- **Rationale**: イシュー本文の期待動作・状態遷移表と完全に一致し、コード変更が最小限
- **Trade-offs**: DB 更新失敗（`mark_failed` エラー）は依然として伝播するが、それはループ継続不可能な障害であるため許容
- **Follow-up**: `mark_succeeded` の呼び出しパスが到達しないことをテストで確認する

## Risks & Mitigations

- GitHub API の一時的障害によりセッションが `failed` になりリトライが増加するリスク — 既存の `RetryPolicy` がカバーしており本修正の範囲外
- `mark_failed` 後にセッションループ変数が後続コードに到達するリスク — 早期 `return Ok(())` によりスコープを抜けるため問題なし

## References

- `src/application/polling/resolve.rs` — 修正対象ファイル（Running フェーズ 行 289–309、Fixing フェーズ 行 316–326）
- `src/application/port/process_run_repository.rs` — `mark_failed` トレイト定義
- `docs/architecture/polling-loop.md` — 状態遷移ルール（イシュー本文抜粋）
