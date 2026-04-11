# Requirements Document

## Project Description (Input)
## Problem

The `observe_pr_for_type` function (src/application/polling/collect.rs:188-203) has a robustness issue in how it discovers the PR to observe. It relies on post-fetch filtering via `.and_then(|r| r.pr_number)` rather than enforcing the filter at the SQL level.

## Code References

**Call sites** (src/application/polling/collect.rs):
- Line 116: `observe_pr_for_type(github, process_repo, id, ProcessRunType::Design)`
- Line 119: `observe_pr_for_type(github, process_repo, id, ProcessRunType::Impl)`

**Current implementation** (lines 188-203):
```rust
async fn observe_pr_for_type<G, P>(
    github: &G,
    process_repo: &P,
    issue_id: i64,
    type_: ProcessRunType,
) -> Result<Option<PrSnapshot>>
where
    G: GitHubClient,
    P: ProcessRunRepository,
{
    // Find latest process run with a pr_number
    let latest = process_repo.find_latest(issue_id, type_).await?;
    let pr_number = match latest.and_then(|r| r.pr_number) {
        Some(n) => n,
        None => return Ok(None),
    };
    // ...
}
```

**Repository query** (src/adapter/outbound/sqlite_process_run_repository.rs:218-224):
```sql
SELECT id, issue_id, type, idx, state, pid, pr_number, causes,
        started_at, finished_at, error_message
 FROM process_runs
 WHERE issue_id = ?1 AND type = ?2
 ORDER BY id DESC LIMIT 1
```

**Architecture spec** (docs/architecture/observations.md:16-17):
```
design_pr:    Option<PrSnapshot>      // type=design で pr_number IS NOT NULL の最新レコードがある場合のみ
impl_pr:      Option<PrSnapshot>      // type=impl で pr_number IS NOT NULL の最新レコードがある場合のみ
```

## Risk Scenario

**Brittle post-fetch filtering**: The current code fetches the latest row by `id DESC LIMIT 1`, then checks `pr_number IS NOT NULL` in Rust. This works today but is fragile:

1. Suppose the latest row by `id` for `(issue_id=1, type=Design)` has `pr_number = NULL` (e.g., a stale or failed run).
2. The function returns `None`, even though an older row with `type=Design` and `pr_number IS NOT NULL` exists.
3. This violates the spec requirement that `design_pr` should be derived from "the latest record where `type=design AND pr_number IS NOT NULL`", not "the latest record where `type=design` followed by a Rust-level null check".

**Schema evolution risk**: If a future code change adds a `design_fix` type row with `pr_number` set between the latest real `design` row and the query execution, and if the schema or stored data accidentally conflates type filters, `LIMIT 1` may return the wrong type.

**Compliance risk**: The spec explicitly states `pr_number IS NOT NULL` as a filter condition. The current SQL does not enforce it—it relies on post-fetch filtering, making it easy to regress.

## Fix Approach

Add a dedicated query in `ProcessRunRepository::find_latest_with_pr_number()` that enforces both filters at the SQL level:

```sql
SELECT id, issue_id, type, idx, state, pid, pr_number, causes,
        started_at, finished_at, error_message
 FROM process_runs
 WHERE issue_id = ?1 AND type = ?2 AND pr_number IS NOT NULL
 ORDER BY id DESC LIMIT 1
```

Then update `observe_pr_for_type` to:
1. Call `process_repo.find_latest_with_pr_number(issue_id, type_).await?`
2. If `None`, return `Ok(None)` immediately (no need to check `pr_number` in Rust)
3. If `Some(run)`, proceed to fetch the PR from GitHub

This ensures:
- The SQL query is the source of truth
- The Rust code is simpler (no `.and_then(|r| r.pr_number)`)
- Future maintainers cannot accidentally break the invariant

## Test Plan

- **Unit test**: Mock the new `find_latest_with_pr_number()` to return `None`, verify `observe_pr_for_type` returns `Ok(None)`.
- **Unit test**: Mock the new method to return a row with `pr_number = Some(42)`, verify it calls `github.get_pr_status(42)`.
- **Integration test**: Populate the database with mixed rows (some with `pr_number`, some without) for the same `(issue_id, type)`, verify the query skips null rows.
- **Regression test**: Ensure existing `observe_github_issue` and other snapshot builders still pass.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

## Requirements

## はじめに

本ドキュメントは、`observe_pr_for_type` 関数が PR を特定する際のロジックを SQLレベルで正確に実装するための要件を定義する。現状では `pr_number IS NOT NULL` フィルタがRustコード側で後処理として行われており、アーキテクチャ仕様に反する脆弱な実装となっている。本修正により、SQLクエリが真実の源となり、Rustコードが簡潔化され、将来の回帰リスクが排除される。

## Requirements

### Requirement 1: ProcessRunRepository トレイトへの新メソッド追加

**Objective:** 開発者として、`pr_number IS NOT NULL` 条件をSQLレベルで強制するポートメソッドを追加したい。そうすることで、アーキテクチャ仕様との整合性を保ち、将来の誤実装を防止できる。

#### Acceptance Criteria

1. The `ProcessRunRepository` trait shall provide a `find_latest_with_pr_number` method with the same signature as `find_latest` (taking `issue_id: i64` and `type_: ProcessRunType`) that returns `Result<Option<ProcessRun>>`.
2. When `find_latest_with_pr_number` is called, the system shall return only the most recent `ProcessRun` record for the given `(issue_id, type_)` pair where `pr_number IS NOT NULL`.
3. If no record satisfying the `pr_number IS NOT NULL` condition exists for the given `(issue_id, type_)`, the system shall return `Ok(None)`.

### Requirement 2: SQLite アダプターへの実装

**Objective:** 開発者として、`find_latest_with_pr_number` をSQLiteアダプターに実装したい。そうすることで、`pr_number IS NOT NULL` フィルタがデータベースクエリレベルで実行されることを保証できる。

#### Acceptance Criteria

1. The `SqliteProcessRunRepository` shall implement `find_latest_with_pr_number` using a SQL query that includes `AND pr_number IS NOT NULL` in the WHERE clause, ordered by `id DESC LIMIT 1`.
2. The adapter shall follow the existing `spawn_blocking` + `Arc<Mutex<Connection>>` pattern used in other methods of the same struct.
3. If the SQL query returns no rows, the system shall return `Ok(None)` without error.

### Requirement 3: observe_pr_for_type の簡潔化

**Objective:** 開発者として、`observe_pr_for_type` 関数を `find_latest_with_pr_number` を使用するよう更新したい。そうすることで、Rustレベルの後処理フィルタを除去し、SQLを唯一の真実の源とできる。

#### Acceptance Criteria

1. When `observe_pr_for_type` is called, the system shall invoke `find_latest_with_pr_number` instead of `find_latest`.
2. When `find_latest_with_pr_number` returns `None`, the system shall return `Ok(None)` immediately without performing Rust-level null checks on `pr_number`.
3. When `find_latest_with_pr_number` returns `Some(run)`, the system shall use `run.pr_number` directly (which is guaranteed non-null by the query) to call `github.get_pr_status`.
4. The system shall preserve the existing 404 handling logic unchanged.

### Requirement 4: テスト

**Objective:** 開発者として、新しいメソッドとその呼び出しに対するユニットテストおよびインテグレーションテストを追加したい。そうすることで、回帰を防止し、仕様の正しさを検証できる。

#### Acceptance Criteria

1. When `find_latest_with_pr_number` is mocked to return `None`, the unit test shall verify that `observe_pr_for_type` returns `Ok(None)`.
2. When `find_latest_with_pr_number` is mocked to return `Some(run)` with `pr_number = Some(42)`, the unit test shall verify that `github.get_pr_status(42)` is called.
3. The integration test shall populate `process_runs` with mixed rows (some with `pr_number`, some without) for the same `(issue_id, type_)` pair and verify that `find_latest_with_pr_number` skips NULL rows and returns the correct non-null record.
4. The system shall continue to pass all existing tests for `observe_github_issue` and other snapshot builders after this change.
