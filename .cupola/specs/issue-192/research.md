# リサーチ・設計判断ノート

---
**Purpose**: 調査結果・アーキテクチャ検討・設計判断の根拠を記録する。

---

## サマリー

- **Feature**: issue-192 — PostCiFixLimitComment / RejectUntrustedReadyIssue の i18n 対応とエラーログ修正
- **Discovery Scope**: Simple Addition（既存パターンへの準拠修正）
- **Key Findings**:
  - 2つのエフェクトハンドラが `rust_i18n` パターンから外れていることを確認
  - ベストエフォートエラーログ規約が `execute_effects()` (lines 76-81) で確立されており、`RejectUntrustedReadyIssue` はこれに準拠していない
  - `locales/en.yml` と `locales/ja.yml` の既存キー構造・命名規約を確認

## リサーチログ

### 現行コードの調査

- **Context**: `execute.rs` における2つのエフェクトハンドラの実装確認
- **Sources Consulted**: `src/application/polling/execute.rs`, `locales/en.yml`, `locales/ja.yml`
- **Findings**:
  - `PostCiFixLimitComment` (lines 173-179): `format!()` でハードコード英語メッセージ
  - `RejectUntrustedReadyIssue` (lines 157-171): ハードコード英語 + `let _ = github.comment_on_issue(...)` でエラー握り潰し
  - `PostCompletedComment`, `PostCancelComment`, `PostRetryExhaustedComment` は全て `rust_i18n::t!()` を使用
  - `execute_effects()` のベストエフォート規約: エラー時は `tracing::warn!()` でログ出力し継続
- **Implications**: 既存パターンに合わせる変更のみで対応可能。新規抽象化・新規トレイトは不要。

### i18n キー命名規約の確認

- **Context**: 新規ロケールキーの名前を既存規約に合わせる
- **Findings**:
  - 全キーは `issue_comment.` プレフィックス配下
  - スネークケース: `retry_exhausted`, `all_completed`, `cleanup_done` 等
  - プレースホルダー: `%{変数名}` 形式（例: `%{count}`, `%{error}`）
- **Implications**:
  - `ci_fix_limit` → `%{max_cycles}` プレースホルダー付き
  - `reject_untrusted` → プレースホルダーなし

### ベストエフォートエラーハンドリング規約の確認

- **Context**: `RejectUntrustedReadyIssue` のエラー処理修正方針
- **Findings**:
  - `execute_effects()` では `best_effort` フラグが `true` の場合、`tracing::warn!()` でログ出力後に処理継続
  - `RejectUntrustedReadyIssue` は `is_best_effort() == true` (テストで確認済み)
  - ただし、エフェクト内部で `let _ = ...` でエラーを無視しており、`execute_effects()` のラッパーに到達しない
  - 修正方針: エフェクト内部で `if let Err(e) = ... { tracing::warn!(...) }` パターンを使用
- **Implications**: `execute_effects()` の外側のベストエフォートラッパーへの依存ではなく、エフェクト内部でエラーをログする設計が適切

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク / 制限 |
|-----------|------|------|--------------|
| 既存パターンへの準拠（採用） | `rust_i18n::t!()` と `tracing::warn!()` を使用 | コードの一貫性、テスト容易性 | なし |
| エフェクト内でのエラー伝播 | `?` でエラーを上位へ伝播 | シンプル | ベストエフォートエフェクトの意味論に反する |

## 設計判断

### Decision: `RejectUntrustedReadyIssue` のコメントエラー処理

- **Context**: `let _ = github.comment_on_issue(...)` パターンはエラーを無声に破棄している
- **Alternatives Considered**:
  1. `?` でエラーを伝播 — ベストエフォートエフェクトには不適切
  2. `if let Err(e) = ... { tracing::warn!(...) }` — 既存パターンに準拠
- **Selected Approach**: `if let Err(e)` + `tracing::warn!()` でログ出力
- **Rationale**: `execute_effects()` のベストエフォートラッパーと対称的であり、可観測性を維持
- **Trade-offs**: エラーログが2層で発生するケースはない（エフェクト内部でのみログ）
- **Follow-up**: ユニットテストで `tracing::warn!()` の呼び出しを検証する

### Decision: `PostCiFixLimitComment` のプレースホルダー変数名

- **Context**: `%{max_cycles}` vs `%{count}` のどちらを使用するか
- **Selected Approach**: `%{max_cycles}` を採用（Issue の提案に従う）
- **Rationale**: `count` は `retry_exhausted` で使用済みであり、`max_cycles` の方が意味が明確

## リスクと緩和策

- ロケールファイルのキー追加漏れ → en.yml と ja.yml の両方を同時に更新する
- プレースホルダー名の不一致 → ユニットテストでプレースホルダー補間を検証する

## 参照

- `src/application/polling/execute.rs` — 修正対象ファイル
- `locales/en.yml`, `locales/ja.yml` — ロケールファイル
- `rust-i18n` crate — i18n マクロの使用法
