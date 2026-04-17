# リサーチ・設計判断ログ

---
**目的:** ディスカバリフェーズで得た知見、アーキテクチャ上の調査結果、および設計の根拠を記録する。
---

## サマリー

- **機能名:** `sqlite_helpers` 共通モジュール抽出（issue-264）
- **ディスカバリスコープ:** Simple Addition（コードリファクタリング）
- **主要知見:**
  - `parse_sqlite_datetime` は `sqlite_issue_repository.rs` と `sqlite_execution_log_repository.rs` の両方に同一実装が存在する（真の重複）
  - `str_to_state` は `sqlite_issue_repository.rs` に `pub fn` として定義され、`sqlite_execution_log_repository.rs` が兄弟モジュールとして直接 `use super::sqlite_issue_repository::str_to_state` でインポートしている（不適切な兄弟依存）
  - `sqlite_process_run_repository.rs` は独自の `parse_datetime`（RFC3339 も対応）を持つため、今回の対象外とする

## リサーチログ

### 重複箇所の調査

- **文脈:** Issue #264 の「重複コード」の実態を把握するためにコードを読んだ
- **参照元:** `src/adapter/outbound/sqlite_issue_repository.rs`、`src/adapter/outbound/sqlite_execution_log_repository.rs`、`src/adapter/outbound/sqlite_process_run_repository.rs`
- **知見:**
  - `parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>>` — 両ファイルに完全同一のシグネチャ・実装
    - フォーマット: `%Y-%m-%d %H:%M:%S`（SQLite の `datetime('now')` 出力形式）
    - エラー: `rusqlite::Error::InvalidColumnType`
  - `str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State>` — `sqlite_issue_repository.rs` に `pub fn` として定義。`sqlite_execution_log_repository.rs` は `use super::sqlite_issue_repository::str_to_state` で参照
  - `sqlite_issue_repository.rs` には `parse_rfc3339_datetime` も存在するが、同ファイル内の `row_to_issue` からのみ使われるため移動対象外
  - `sqlite_process_run_repository.rs` の `parse_datetime` は RFC3339 フォールバックあり（異なる実装）—今回の対象外

- **含意:** `str_to_state` の移動により、`sqlite_execution_log_repository.rs` の兄弟依存が解消される。`parse_sqlite_datetime` の移動により真の重複が排除される。

### アクセス可視性の検討

- **文脈:** 新モジュールのヘルパー関数をどの可視性で公開するか
- **知見:**
  - 現在の `str_to_state` は `pub` だが、`src/adapter/outbound/` 外から使われているかを確認
  - `sqlite_issue_repository.rs` のテストコード内: `use super::*;` で親モジュールをまとめて参照している。したがって、新モジュール移動後の参照可否は、テスト自身が `use super::sqlite_helpers::str_to_state` を追加するかどうかではなく、親モジュール側で `use super::sqlite_helpers::str_to_state;` を通じて `super::*` で拾える形を維持するかに依存する
  - `sqlite_process_run_repository.rs` は `str_to_state` を使っていない
  - `src/` 内の他ファイルで `str_to_state` を参照している箇所なし（grep 確認）
  - → `pub(super)` とすることで `adapter/outbound` 内のみからアクセス可能とし、外部公開を最小化できる

## アーキテクチャパターン評価

| 選択肢 | 説明 | 利点 | リスク・制限 | メモ |
|--------|------|------|-------------|------|
| `sqlite_helpers.rs` を新規作成 | `adapter/outbound/` 内に専用ヘルパーモジュールを追加 | 責務が明確、名前が意図を表す | 新ファイル追加のオーバーヘッド（小） | 今回採用 |
| `sqlite_connection.rs` に追加 | 既存の接続管理ファイルに関数を追加 | 新ファイル不要 | 接続管理と型変換の責務が混在、単一責任原則に反する | 不採用 |

## 設計判断

### 判断: `sqlite_helpers.rs` の新規作成

- **文脈:** 共通ヘルパー関数の置き場所
- **検討した代替案:**
  1. `sqlite_connection.rs` に追加 — 既存ファイルを使うが責務混在
  2. 新規 `sqlite_helpers.rs` を作成 — ファイルが増えるが単一責任を維持
- **採用アプローチ:** `src/adapter/outbound/sqlite_helpers.rs` を新規作成し、`str_to_state` と `parse_sqlite_datetime` を配置する
- **根拠:** Issue の提案にも「新規 `sqlite_helpers.rs`」が明示されており、接続管理（`SqliteConnection`）とデータ変換（型変換関数）の責務を分離することが Clean Architecture の単一責任原則に沿う
- **トレードオフ:** ファイル数が 1 増えるが、各モジュールの役割が明確になる
- **確認事項:** `mod.rs` へのモジュール宣言追加を忘れないこと

### 判断: `pub(super)` による可視性制限

- **文脈:** ヘルパー関数を外部に公開する必要があるか
- **採用アプローチ:** `pub(super)` を使い `adapter/outbound` 内のみからアクセス可能とする
- **根拠:** 現在 `str_to_state` は `pub` だが、実際には `adapter/outbound` 外から使われていない。最小公開原則に従い可視性を絞る
- **トレードオフ:** テストコードは `use super::*;` を使用しているため、親モジュールが `use super::sqlite_helpers::str_to_state;` を通じて `str_to_state` を可視化する必要がある。テスト自体への変更は不要

## リスクと軽減策

- 親モジュールで `use super::sqlite_helpers::str_to_state;` を再エクスポートし忘れた場合、テストが `use super::*;` で `str_to_state` を解決できなくなる → `cargo test` で即座に検出可能
- `str_to_state` の `pub` が外部から参照されていた場合のコンパイルエラー → grep による事前確認済み（参照なし）

## 参考文献

- Issue #264 (Split from #135)
- Steering: `tech.md` — Clean Architecture レイヤー規約
- Steering: `structure.md` — モジュール命名規約とコード組織原則
