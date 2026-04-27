# v0001-initial fixture

## 作成バージョン

cupola v0.1.0 相当のスキーマ（`last_pr_review_submitted_at` および `body_hash` カラム追加前）

## 含まれるテーブル

| テーブル名 | 行数 | 備考 |
|-----------|------|------|
| issues | 2 | 下記の行を含む |
| process_runs | 0 | 空テーブル |
| execution_log | 0 | 空テーブル |

## issues テーブルのデータ

| github_issue_number | state | feature_name | weight |
|--------------------|-------|--------------|--------|
| 1 | idle | issue-1 | medium |
| 2 | design_running | issue-2 | heavy |

## issues テーブルのスキーマ（fixture 作成時点）

`last_pr_review_submitted_at` および `body_hash` カラムを **含まない** 旧スキーマ。
`init_schema()` を適用することでこれらのカラムが追加され、現行スキーマへ移行する。

## 検証ポイント

- `init_schema()` を 2 回適用してもエラーが発生しないこと（idempotency）
- マイグレーション後も既存行の `state` 値が保全されること
- 追加カラム `last_pr_review_submitted_at` が NULL になること
- `init_schema()` 後のスキーマが `snapshots/current-schema.sql` と一致すること

## 生成方法

`tests/migrations/mod.rs` の `generate_v0001_fixture` (#[ignore] テスト) を実行:

```bash
devbox run -- cargo test --test migrations -- --ignored generate_v0001_fixture
```
