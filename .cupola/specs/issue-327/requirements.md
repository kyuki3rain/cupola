# 要件定義: DB マイグレーション回帰テストの実装

## Introduction

`docs/tests/migration-test.md` に DB マイグレーションテストの完全な仕様（ディレクトリ構造・fixture 作成手順・テストテンプレート・CI 組み込み・運用ルール）が記載されているが、実装が存在しない。

現状の問題:
- `adapter/outbound/sqlite_connection.rs` の `init_schema` は idempotent に設計されているが、その保証を裏付けるテストが存在しない
- `ALTER TABLE ADD COLUMN` や state 値の rename など、スキーマ変更で過去の DB が壊れるリスクが検出不能
- `SqliteConnection::dump_schema()` テスト用ヘルパーが存在しない
- `tests/migrations/` ディレクトリ、`fixtures/`、`snapshots/` が存在しない

本要件定義では `docs/tests/migration-test.md` の section 8「最初の TODO」を順次実装する。

## Requirements

### Requirement 1: `dump_schema()` テスト用ヘルパーの追加

**Objective:** テスト作成者として、`SqliteConnection` から現在のスキーマ定義を文字列として取得する手段が欲しい。これにより `current-schema.sql` スナップショットとの比較テストが実装できる。

#### Acceptance Criteria

1. When [`dump_schema()` が呼ばれた], the [`SqliteConnection`] shall [`sqlite_master` からすべての CREATE TABLE / CREATE INDEX 文を ORDER BY name で取得して連結した文字列を返す]
2. The [`dump_schema()` メソッド] shall [`#[cfg(test)]` 修飾により、テストコードからのみ呼び出せる形で公開される]
3. When [同一スキーマを持つ 2 つの `SqliteConnection` で `dump_schema()` を呼んだ], the [戻り値] shall [同一の文字列である]
4. When [`init_schema()` 適用後に `dump_schema()` を呼んだ], the [戻り値] shall [`last_pr_review_submitted_at` および `body_hash` カラムを含む issues テーブル定義を返す]

### Requirement 2: マイグレーションテスト基盤の整備

**Objective:** 品質保証担当として、古いスキーマの DB ファイルを現行バイナリで開いても問題なく動作することを継続的に検証できるテスト基盤が欲しい。

#### Acceptance Criteria

1. The [テスト基盤] shall [`tests/migrations/mod.rs`・`tests/migrations/fixtures/`・`tests/migrations/snapshots/` の構成を持つ]
2. The [`v0001-initial.db`] shall [`last_pr_review_submitted_at` および `body_hash` カラムを持たない issues テーブルを含む SQLite DB ファイルである]
3. The [`v0001-initial.db`] shall [`github_issue_number=1 (state='idle', feature_name='issue-1', weight='medium')` および `github_issue_number=2 (state='design_running', feature_name='issue-2', weight='heavy')` の 2 行を issues テーブルに含む]
4. The [`v0001-initial.README.md`] shall [fixture の作成バージョン、含まれるテーブル・行数、検証ポイントを日本語で記述する]
5. The [`snapshots/current-schema.sql`] shall [フレッシュな in-memory DB に `init_schema()` を適用した後の `dump_schema()` 出力を正規化した内容で保存する]

### Requirement 3: `migrate_v0001_is_idempotent` テストの実装

**Objective:** 開発者として、`v0001-initial.db` fixture に `init_schema()` を 2 回適用してもデータが保全され、新カラムがデフォルト値で補完されることを自動テストで証明できることを望む。

#### Acceptance Criteria

1. When [`v0001-initial.db` を読み込んで `init_schema()` を 2 回呼んだ], the [テスト] shall [エラーもパニックも発生させずに正常終了する]
2. When [マイグレーション後], the [テスト] shall [`github_issue_number=1` の `state` が `'idle'` であることを検証する]
3. When [マイグレーション後], the [テスト] shall [`github_issue_number=2` の `state` が `'design_running'` であることを検証する]
4. When [マイグレーション後], the [テスト] shall [`last_pr_review_submitted_at` カラムが存在し、`github_issue_number=1` の値が NULL であることを検証する]
5. The [テスト名 `migrate_v0001_is_idempotent`] shall [失敗時にどの fixture で落ちたかを名前から即座に判別可能にする]

### Requirement 4: `fixture_reaches_current_schema` テストの実装

**Objective:** 開発者として、`v0001-initial.db` への `init_schema()` 適用後のスキーマが `current-schema.sql` と完全一致することを自動テストで証明できることを望む。

#### Acceptance Criteria

1. When [`v0001-initial.db` を読み込んで `init_schema()` を呼んだ], the [テスト] shall [`dump_schema()` の結果を `normalize_schema()` で正規化して `current-schema.sql` の正規化済み内容と比較する]
2. If [スキーマが一致しない], the [テスト] shall [差分を含む分かりやすいエラーメッセージで失敗する]
3. The [`normalize_schema()` ヘルパー] shall [CRLF → LF 変換・末尾空白除去・空行除去を行うことで環境差異を吸収する]
4. The [テスト名 `fixture_reaches_current_schema`] shall [失敗時にどの fixture で落ちたかを名前から判別可能にする]

### Requirement 5: `Cargo.toml` への CI 組み込み

**Objective:** CI 管理者として、マイグレーションテストが `cargo test` の一部として常に自動実行されることを望む。

#### Acceptance Criteria

1. The [`Cargo.toml`] shall [`[[test]] name = "migrations" path = "tests/migrations/mod.rs"` セクションを含む]
2. When [`devbox run -- cargo test --test migrations` を実行した], the [テストスイート] shall [秒単位で green になる]
3. The [テスト] shall [`cargo test` （引数なし）でも自動的に実行される]

### Requirement 6: 運用ドキュメントの更新

**Objective:** リリース担当者として、スキーマ変更時に fixture を追加する必要があることをドキュメントで確認でき、手順を把握できることを望む。

#### Acceptance Criteria

1. The [`CHANGELOG.md`] shall [`[Unreleased]` セクションにマイグレーション回帰テスト追加の記録を含む]
2. The [`CHANGELOG.md`] shall [スキーマ変更時は `tests/migrations/fixtures/` に新規 fixture を追加する必要がある旨を `[Unreleased]` セクション内に注記する]
