# 調査・設計決定ログ

---
**Feature**: `issue-327` — DB マイグレーション回帰テスト実装
**Discovery Scope**: Extension (既存アダプター層への追加)
**Key Findings**:
- `SqliteConnection` は `Arc<Mutex<Connection>>` を持ち `conn_lock()` で rusqlite を直接操作できる
- `tempfile` は既に `[dev-dependencies]` に存在する
- `Cargo.toml` に `[[test]]` セクションはなく、既存統合テストは `integration_test.rs` / `scenarios.rs` の 2 ファイルのみ
- `init_schema()` は `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN` (エラー無視) + state rename の 3 段構成でマイグレーションを実現している

---

## 調査ログ

### SqliteConnection の現行 API

- **Context**: dump_schema() と load_fixture() が既存 API と整合するか確認する必要があった。
- **調査箇所**: `src/adapter/outbound/sqlite_connection.rs`
- **Findings**:
  - `pub fn open(path: &Path) -> Result<Self>` — ファイルパスから DB を開く
  - `pub fn open_in_memory() -> Result<Self>` — インメモリ DB
  - `pub fn conn_lock() -> MutexGuard` — Mutex ガードを返す (パニック on poison)
  - `pub fn conn() -> &Arc<Mutex<Connection>>` — Arc 参照
  - `pub fn init_schema() -> Result<()>` — 冪等 schema 適用
  - `#[cfg(test)] mod tests` が既に存在し `open_in_memory` + `init_schema` のテストパターンが確立
- **Implications**: `dump_schema()` は `#[cfg(test)]` impl ブロックに追加し `conn_lock()` を通じて `sqlite_master` を照会すればよい。

### 既存マイグレーション実装の詳細

- **Context**: v0001 フィクスチャがどの「古さ」を模倣すべきかを決定するため。
- **調査箇所**: `src/adapter/outbound/sqlite_connection.rs::init_schema()`
- **Findings**:
  - `CREATE TABLE IF NOT EXISTS issues` で 12 カラム (id, github_issue_number, state, feature_name, weight, worktree_path, ci_fix_count, ci_fix_limit_notified, close_finished, consecutive_failures_epoch, created_at, updated_at) を一括定義
  - `run_add_column_migration` で以下のカラムを後付け: `feature_name`, `weight`, `ci_fix_count`, `ci_fix_limit_notified`, `close_finished`, `consecutive_failures_epoch`, `last_pr_review_submitted_at`, `body_hash`
  - `process_runs` テーブルと `execution_log` テーブルも `CREATE TABLE IF NOT EXISTS` で追加
  - state rename: `initialized` → `initialize_running`
  - feature_name backfill: NULL を `'issue-' || github_issue_number` で埋める
  - 既存テスト `migration_adds_weight_column_to_existing_db` が「legacy issues table without weight column」を再現するパターンを示している
- **Implications**: v0001 フィクスチャは `weight`, `ci_fix_count` などのカラムが存在しない古い `issues` テーブルを持つ DB とする。`process_runs` / `execution_log` は省略して最大のマイグレーションパスを踏む。

### テスト登録パターン

- **Context**: `[[test]]` セクションの有無と正しい追記位置を確認するため。
- **調査箇所**: `Cargo.toml`
- **Findings**: `[[test]]` セクションは存在しない。既存の `tests/integration_test.rs` / `tests/scenarios.rs` は Cargo のデフォルト探索 (`tests/` ディレクトリ) で自動登録されている。
- **Implications**: `tests/migrations/mod.rs` はデフォルト探索対象外 (サブディレクトリ) のため、明示的に `[[test]]` セクションを追加する必要がある。

### tempfile クレートの利用可能性

- **Context**: load_fixture がフィクスチャをコピーして安全に操作するため。
- **調査箇所**: `Cargo.toml` の `[dev-dependencies]`
- **Findings**: `tempfile = "3"` が既に存在する。
- **Implications**: 新規依存関係の追加は不要。

---

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク・制限 | 備考 |
|-----------|-----|------|-------------|------|
| `#[cfg(test)]` impl ブロック直接追加 | `sqlite_connection.rs` に dump_schema を追加 | 既存パターンと整合、ボイラープレート最小 | なし | 採用 |
| 別 trait として抽象化 | `DumpSchema` trait を定義 | テスト以外での再利用可能 | 過剰設計、テスト専用ヘルパーに trait は不要 | 不採用 |
| proc-macro テストヘルパー | derive で自動生成 | 将来拡張性 | 過剰設計 | 不採用 |

---

## 設計決定

### 決定: dump_schema() の SQL クエリと出力形式

- **Context**: スキーマ比較を決定論的にする必要がある。SQLite の `sqlite_master` は挿入順に結果を返す。
- **検討した代替案**:
  1. `SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND sql IS NOT NULL ORDER BY name` — 名前順のみ
  2. `ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, name ASC` — テーブル優先、名前昇順
- **選択したアプローチ**: テーブルを先にし名前昇順でソート。各 SQL 文を `";\n"` で連結して末尾に `";"` を付与。
- **根拠**: `current-schema.sql` と dump 結果を同じ生成関数で作るため、ソート順が一致さえすれば比較は確実。
- **トレードオフ**: テーブル先順は直感的でレビューしやすい。一方、インデックスを後に置くので対応するテーブルと並ばない。
- **Follow-up**: `current-schema.sql` は `dump_schema()` を呼ぶテストヘルパーで生成するため、常に整合する。

### 決定: v0001-initial.db フィクスチャの作成方法

- **Context**: フィクスチャはバイナリ SQLite ファイルとして git commit する必要がある。手作業 or プログラム生成の選択。
- **検討した代替案**:
  1. `cupola init` を旧タグで checkout して手動生成 — 真の歴史的 DB だが旧タグが存在しない
  2. `#[ignore]` 付きテスト関数で programmatically 生成 — 再現可能でコードに記録が残る
  3. sqlite3 CLI スクリプトで手動作成 — シンプルだがコード上に手順が残らない
- **選択したアプローチ**: `tests/migrations/mod.rs` に `#[test] #[ignore] fn create_v0001_fixture()` を実装し、旧スキーマ (weight 等なし) でテーブルを作成し行を挿入後、`tests/migrations/fixtures/v0001-initial.db` に書き出す。実装者が一度 `cargo test -- --ignored create_v0001_fixture` を実行してバイナリを生成し、git commit する。
- **根拠**: フィクスチャ生成手順がコードとして残り、将来の再生成や審査が容易。`#[ignore]` により通常テスト実行には含まれない。
- **トレードオフ**: 実装者が明示的に ignore テストを一度実行する手順が必要。しかし手順書に記載するため問題ない。

### 決定: normalize_schema の実装範囲

- **Context**: `migration-test.md` section 7 は CRLF→LF と末尾空白除去を要求している。
- **選択したアプローチ**: `fn normalize_schema(sql: &str) -> String` を `tests/migrations/mod.rs` に自由関数として実装。`replace("\r\n", "\n")` → 行分割 → `trim_end()` → 空行除去 → join。
- **根拠**: テスト専用の最小実装。外部公開不要。
- **トレードオフ**: 将来さらに精密な正規化 (コメント除去、空白統一) が必要になった場合は拡張する。

---

## リスクと緩和策

- **クロスプラットフォーム SQLite バイナリ差異** — CI と開発環境の sqlite バージョン差で `sqlite_master.sql` テキストが微妙に異なる可能性。緩和策: `normalize_schema` でホワイトスペース差異を吸収。`current-schema.sql` を CI で生成するのではなく、同じコードパスの `dump_schema()` で生成することで一致を保証する。
- **フィクスチャ git add 忘れ** — バイナリファイルを add し忘れると CI で fixture not found になる。緩和策: tasks.md に明示。
- **`#[cfg(unix)]` 依存の問題** — `src/lib.rs` の依存は `#[target.'cfg(unix)'.dependencies]` 配下だが、テスト対象の `SqliteConnection` は `rusqlite` (unix only) を使用している。緩和策: フィクスチャテストも unix 環境 (CI) で実行するため問題なし。

---

## 参照

- `docs/tests/migration-test.md` — マイグレーションテスト仕様書 (255 行)
- `src/adapter/outbound/sqlite_connection.rs` — SqliteConnection 実装
- `Cargo.toml` — 依存関係と dev-dependencies
