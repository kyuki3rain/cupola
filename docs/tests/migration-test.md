# DB マイグレーション回帰テスト

cupola の `.cupola/cupola.db` は SQLite で、スキーマは
`adapter/outbound/sqlite_connection.rs` の `init_schema` が idempotent に
適用する設計になっている。ユーザーが古いバージョンの cupola で作った DB を
新しいバイナリで開いても問題なく動くかどうかは、E2E ではなく**スナップ
ショット fixture を使った単体テスト**で担保する。

本ドキュメントは:

- なぜ E2E ではなく単体テストなのか
- fixture の作り方と置き場所
- 新しいリリース時の運用
- テストの書き方

を定義する。E2E 側の方針は [e2e-plan.md](./e2e-plan.md) を参照。

## 1. なぜ単体テストか

マイグレーションの本質は「古い schema の DB ファイル → `init_schema` を走らせる
→ 現在の期待 schema と一致するか」の三点セット。これは純粋に SQLite の世界で
完結するので:

- GitHub も Claude も必要ない
- `tokio` も最小限（`spawn_blocking` だけ）
- 秒以下で決定的に回る
- CI のコミットごとに気兼ねなく走らせられる

E2E（ephemeral 新規 repo）で「マイグレーションも通ったらしい」と副作用的に
観測するより、fixture 固定の単体テストで「v1 → v_current、v2 → v_current、
v3 → v_current」と網羅的に証拠を残す方が価値が高い。

## 2. ディレクトリ構成

```
tests/
└── migrations/
    ├── mod.rs                      ← rust エントリ（#[test] を列挙）
    ├── fixtures/
    │   ├── v0001-initial.db        ← 最初の schema（手動で保存）
    │   ├── v0001-initial.README.md ← どのリリースのものか
    │   ├── v0002-add-process-runs.db
    │   ├── v0002-add-process-runs.README.md
    │   ├── v0003-add-close-finished.db
    │   └── v0003-add-close-finished.README.md
    └── snapshots/
        └── current-schema.sql      ← 現在の期待 schema を sqldump で保存
```

- `fixtures/*.db` は**バイナリ**だが SQLite 1 ファイルなので数 KB 程度。
  git にそのまま commit する。
- `*.README.md` は **手動で書く**。fixture がどのリリースの DB か、
  どのように作られたか、何を検証したいかを記述する。
- `snapshots/current-schema.sql` は `init_schema` が吐く最新スキーマの
  スナップショット。fixture と比較する基準として使う。テストが失敗したとき
  `cargo insta review` のように差分を目視で確認できるようにする。

## 3. fixture の作り方

新しい schema を導入したバージョンをリリースする直前に **1 ファイル追加する**
運用にする。

```bash
# 1. 現行 main (古い schema を持つ版) を checkout
git checkout <tag-of-previous-release>

# 2. 一時ディレクトリで cupola init を走らせる
mkdir /tmp/migration-fixture && cd /tmp/migration-fixture
devbox run -- cargo run --release -- init

# 3. いくつかの typical な行を入れておく（空 DB だと分岐を踏まない）
sqlite3 .cupola/cupola.db <<'SQL'
INSERT INTO issues (github_issue_number, state, feature_name, weight, created_at, updated_at)
VALUES (1, 'idle', 'issue-1', 'medium', datetime('now'), datetime('now')),
       (2, 'design_running', 'issue-2', 'heavy', datetime('now'), datetime('now'));
SQL

# 4. ファイルを fixture ディレクトリへ
cp .cupola/cupola.db \
   $CUPOLA_REPO/tests/migrations/fixtures/v000X-description.db

# 5. README を書く
cat > $CUPOLA_REPO/tests/migrations/fixtures/v000X-description.README.md <<'EOF'
# v000X — 説明

- 作成元: tag `v0.Y.Z`
- 含まれるテーブル: issues, process_runs
- 含まれる行数: issues=2, process_runs=0
- 検証したいポイント: (例) `close_finished` カラムが無い状態から、
  init_schema が ALTER TABLE ADD COLUMN でカラムを追加できるか。
EOF

# 6. checkout を戻して、最新 main で snapshot を更新
cd $CUPOLA_REPO
git checkout main
devbox run -- cargo run --release -- init  # 一時ディレクトリで
sqlite3 /tmp/latest.db .schema > tests/migrations/snapshots/current-schema.sql
```

**命名規則**: `v<4 桁通番>-<slug>.db`
- 通番は ASC な整数で monotonic に増やす
- slug は何をテストしたいかを短く表す（例: `initial`, `add-process-runs`,
  `add-close-finished`, `rename-state-key`）

## 4. テストの書き方

### 4.1 基本形

```rust
// tests/migrations/mod.rs
use cupola::adapter::outbound::sqlite_connection::SqliteConnection;

fn load_fixture(name: &str) -> SqliteConnection {
    let src = format!("tests/migrations/fixtures/{name}.db");
    let dst = tempfile::NamedTempFile::new().unwrap();
    std::fs::copy(&src, dst.path()).unwrap();
    SqliteConnection::open(dst.path()).expect("open fixture")
}

/// v0001 → 現行 schema: init_schema を 2 回呼んでも idempotent で、
/// 既存行が失われず、新カラムがデフォルト値で埋まること。
#[test]
fn migrate_v0001_is_idempotent() {
    let conn = load_fixture("v0001-initial");

    // 1 回目
    conn.init_schema().expect("first init_schema");
    // 2 回目（idempotent の確認）
    conn.init_schema().expect("second init_schema");

    // 既存行の保全
    let rows: Vec<(i64, String)> = conn.query_all(
        "SELECT github_issue_number, state FROM issues ORDER BY github_issue_number"
    ).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[0].1, "idle");

    // 新カラムのデフォルト値
    let close_finished: i64 = conn.query_one(
        "SELECT close_finished FROM issues WHERE github_issue_number = 1"
    ).unwrap();
    assert_eq!(close_finished, 0);
}
```

### 4.2 状態キーの rename を含む fixture

`Initialized` → `InitializeRunning` のような rename は `UPDATE issues SET state=...`
の migration が走っていることを確認する:

```rust
#[test]
fn migrate_renames_initialized_to_initialize_running() {
    let conn = load_fixture("v0001-initial"); // state='initialized' を含む
    conn.init_schema().unwrap();

    let count: i64 = conn.query_one(
        "SELECT COUNT(*) FROM issues WHERE state = 'initialize_running'"
    ).unwrap();
    assert!(count >= 1, "v0001 fixture should have at least one initialize_running row");

    let stale: i64 = conn.query_one(
        "SELECT COUNT(*) FROM issues WHERE state = 'initialized'"
    ).unwrap();
    assert_eq!(stale, 0, "no 'initialized' rows should remain");
}
```

### 4.3 schema スナップショット比較

```rust
#[test]
fn fixture_reaches_current_schema() {
    let conn = load_fixture("v0001-initial");
    conn.init_schema().unwrap();

    let expected = std::fs::read_to_string(
        "tests/migrations/snapshots/current-schema.sql"
    ).unwrap();
    let actual = conn.dump_schema();

    // 順序依存を避けたい場合はソート + 正規化
    let expected_norm = normalize_schema(&expected);
    let actual_norm = normalize_schema(&actual);
    assert_eq!(actual_norm, expected_norm, "schema mismatch after migration");
}
```

`dump_schema()` は `sqlite_master` を引いて `CREATE TABLE` 文を全部取り出す
ヘルパーとして `SqliteConnection` に追加する（テストでだけ使う cfg(test) 公開）。

## 5. 新しい schema バージョンをリリースする時のフロー

1. **実装**: `init_schema` に新しい `ALTER TABLE` 等を追加
2. **`current-schema.sql` を更新**: 最新 main で `cupola init` → `.schema` を吐き直す
3. **直前バージョンの fixture を新規追加**: section 3 の手順で `v000X-<slug>.db` を作る
4. **テストを追加**: section 4 のテンプレートで 1〜3 本追加
5. **既存 fixture はそのまま残す**: 削除しない。`v0001 → 現行`, `v0002 → 現行`, … と
   全てのバージョンからのマイグレーションが通ることを永久に確認し続ける
6. **CHANGELOG に記載**: schema 変更があった旨と fixture ID を書く

### 5.1 fixture の保守ルール

- **一度 commit した fixture は削除・改変しない**。そのリリースから upgrade
  する人が世界のどこかに居る可能性がある限り、その fixture は必要。
- 古すぎて保守不能になった場合は README に "deprecated since vX" と書いて
  `#[ignore]` を付ける。消すのは最終手段。
- fixture のサイズが大きくなりすぎた場合は Git LFS 移行を検討するが、
  数 MB 以内に収まっているうちは普通に commit でよい。

## 6. CI 組み込み

マイグレーションテストは `cargo test` の一部として**常に走らせる**。
別ターゲット化せず、以下のように整合させる:

```toml
# Cargo.toml
[[test]]
name = "migrations"
path = "tests/migrations/mod.rs"
```

CI での期待:

- `devbox run -- cargo test --test migrations` が秒単位で完了する
- 失敗した場合、どの fixture で落ちたかがテスト名から一目で分かる
- fmt / clippy / 他のテストと同列の扱い（gate として PR マージをブロックする）

## 7. 既知の制約

- **新規リリースごとに手動で fixture を 1 つ作る**のを忘れないこと。
  運用で忘れると「old → current の migration が壊れてもテストでは気づかない」
  事故が起こる。CHANGELOG テンプレートに fixture 追加のチェックボックスを入れる。
- **SQLite のバージョン差**: fixture を作ったときと CI のときで sqlite バイナリ
  のマイナーバージョンが違うと datetime format 等で微妙な差が出ることがある。
  `snapshots/current-schema.sql` の正規化（CRLF → LF、末尾空白除去）を挟む。
- **schema 変更を `init_schema` 外で行わないこと**: 例えば手動の
  `conn.execute("ALTER TABLE …")` をアプリ本体から呼ぶと、再現不能な
  migration になってしまう。schema 変更は必ず `init_schema` の中で
  idempotent に実装する。

## 8. 最初の TODO

本ドキュメント導入時点で fixture は空なので、以下の順で埋める:

- [ ] `SqliteConnection::dump_schema()` を `#[cfg(test)]` で追加する
- [ ] `tests/migrations/mod.rs` と `fixtures/` / `snapshots/` ディレクトリを作成
- [ ] 現行 main を基準に `snapshots/current-schema.sql` を初期化
- [ ] 前リリース (架空の `v0001-initial.db`) の fixture を 1 本作成
- [ ] section 4.1 / 4.3 のテンプレートで最初のテストを 2 本書く
- [ ] `cargo test --test migrations` が green になることを確認
- [ ] CI パイプライン (`cargo test`) に自動組み込まれていることを確認
- [ ] CHANGELOG / リリース手順書に「schema 変更時は fixture を追加する」を追記
