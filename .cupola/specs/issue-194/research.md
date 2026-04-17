# リサーチ & 設計決定

---
**Feature**: issue-194 — polling: DB 更新失敗後のリトライ機構
**Discovery Scope**: Extension（既存エフェクトハンドラへの拡張）
**Key Findings**:
- GitHub `close_issue` API は冪等（閉済み Issue を再度閉じても成功）
- `git worktree remove --force` は冪等（`git_worktree_manager.rs:76` で存在チェック済み）
- `update_state_and_metadata` はトランザクションなし・単一 UPDATE 文・ロールバック不可
- アプリケーション層に非同期リトライヘルパーを追加するのが最もクリーンな設計
---

## Research Log

### execute.rs の現状分析

- **Context**: `CloseIssue`（行 272-284）と `CleanupWorktree`（行 227-269）の実装を確認
- **Findings**:
  - `CloseIssue`: `github.close_issue(n).await?` → `issue_repo.update_state_and_metadata(...).await?` → `issue.close_finished = true` の順
  - DB 更新が `?` で伝播されるため、失敗時はインメモリ更新（最後の行）が実行されない
  - `CleanupWorktree`: `worktree.remove(path)?` → ブランチ削除（best-effort）→ `issue_repo.update_state_and_metadata(...).await?` → `issue.worktree_path = None` の順
  - 同様に DB 失敗でインメモリ更新がスキップされる
- **Implications**: DB 更新の直後にインメモリ更新が来るため、リトライロジックは DB 更新呼び出しのラッパーとして実装すれば最小変更で対応可能

### update_state_and_metadata の実装分析

- **Context**: `sqlite_issue_repository.rs` 行 215-284 を確認
- **Findings**:
  - `tokio::task::spawn_blocking` で SQLite 操作を実行
  - 動的 SQL ビルド（`set_clauses` + `param_values`）で単一 UPDATE 文を発行
  - 失敗時は `anyhow::Error` を返す（`context("update issue failed")`）
  - トランザクションや部分コミットはなし
- **Implications**: リトライは呼び出し側（execute.rs）で行うのが適切。リポジトリ実装を変更する必要はない

### 冪等性の確認

- **CloseIssue (GitHub REST API)**:
  - GitHub REST API spec: 閉済み Issue を再度 close しても HTTP 200 を返す（冪等）
  - `github_rest_client.rs` 行 289-299 の実装を確認済み
  - リトライにより GitHub API が重複実行されても問題なし
- **CleanupWorktree (git worktree remove)**:
  - `git_worktree_manager.rs` 行 76: `if !self.exists(path) { return Ok(()) }` で存在チェック
  - 削除後の再試行も安全

### リトライ設計オプション評価

- **アプリケーション層ヘルパー関数**: `execute.rs` 内に `retry_db_update` 非同期ヘルパーを追加
  - 利点: 既存の Clean Architecture を壊さない、最小変更、テスト容易
  - 欠点: `CloseIssue`・`CleanupWorktree` 以外への拡張が必要になった場合は別途対応
- **IssueRepository ポートレベルのリトライ**: ポートのデフォルト実装としてリトライを追加
  - 利点: 透過的に全呼び出しをリトライ可能
  - 欠点: DB 更新全体がリトライ対象になり、意図しない箇所への影響が懸念
- **選択**: アプリケーション層ヘルパー関数を選択（最小スコープ、影響範囲が明確）

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: アプリ層ヘルパー関数 | `execute.rs` に `retry_db_update` 非同期関数を追加 | 最小変更、テスト容易、Clean Architecture 準拠 | 同様のパターンが他エフェクトに発生した場合は都度追加が必要 | 採用 |
| B: ポートレベルリトライ | `IssueRepository` トレイト実装にリトライを組み込む | 透過的、呼び出し側変更不要 | スコープが広すぎる、意図しない呼び出しもリトライ対象になる | 不採用 |
| C: 現状維持 + ドキュメント | 既知の運用リスクとして文書化 | 変更なし | `CloseIssue` の silent failure が解消されない | 不採用 |

## Design Decisions

### Decision: リトライヘルパーをアプリケーション層に配置

- **Context**: `execute.rs` は `src/application/polling/` 配下にある。リトライロジックをどこに実装するか。
- **Alternatives Considered**:
  1. `execute.rs` 内プライベート関数 — 同ファイル内で完結し変更最小
  2. `src/application/polling/retry.rs` 独立モジュール — 再利用性が高い
  3. アダプター層（`SqliteIssueRepository`）内でラップ — ポート実装内でリトライ
- **Selected Approach**: `execute.rs` 内プライベート非同期ヘルパー関数 `retry_db_update`
- **Rationale**: 変更スコープが最小で、他エフェクトへの影響なし。将来的に複数エフェクトで使うようになれば独立モジュールに昇格できる。
- **Trade-offs**: `CloseIssue`・`CleanupWorktree` 以外のエフェクトに同様の問題が発生した場合、個別対応が必要
- **Follow-up**: リトライが必要なエフェクトが 3 つ以上になったら独立モジュールへのリファクタリングを検討

### Decision: リトライ回数と遅延

- **Context**: Issue で推奨された「3× 指数バックオフ（1s, 2s, 4s）」を採用するか
- **Selected Approach**: 最大 3 回、遅延 1 秒→2 秒→4 秒の指数バックオフ（合計最大 7 秒の追加レイテンシ）
- **Rationale**: DB ロックタイムアウトや一時的なディスクフルは数秒以内に解消するケースが多い。7 秒はポーリングループのレイテンシとして許容範囲内。
- **Trade-offs**: 永続的な DB 障害の場合は 7 秒待ってからエラーが返される

### Decision: インメモリ状態の更新タイミング

- **Context**: DB 更新成功後にインメモリ状態を更新するか、リトライ成功後に更新するか
- **Selected Approach**: DB 更新リトライが最終的に成功した場合のみインメモリ状態を更新。失敗時は更新しない。
- **Rationale**: インメモリ状態と DB の一貫性を保つための既存設計思想を維持する。

## Risks & Mitigations

- DB 障害が 7 秒以上継続する場合、エラーが返されて次のポーリングサイクルまでリトライが延期される — 許容リスク（次サイクルで自動リトライ）
- リトライ中の非同期スリープがポーリングサイクルの合計時間を延ばす — リトライはトップレベルの `tokio::time::sleep` で行い、他 Issue の処理には影響しない設計とする

## References

- `src/application/polling/execute.rs` 行 227-284 — CleanupWorktree / CloseIssue 実装
- `src/adapter/outbound/sqlite_issue_repository.rs` 行 215-284 — `update_state_and_metadata` 実装
- `src/adapter/outbound/github_rest_client.rs` 行 289-299 — `close_issue` 冪等性確認
- `src/adapter/outbound/git_worktree_manager.rs` 行 73-81 — `remove` 冪等性確認
- `docs/architecture/effects.md` 行 88-191 — CleanupWorktree / CloseIssue アーキテクチャ説明
