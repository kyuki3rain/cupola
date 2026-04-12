# Implementation Plan

- [ ] 1. ProcessRunRepository トレイトに find_latest_with_pr_number メソッドを追加する
  - `find_latest` と同じシグネチャで `pr_number IS NOT NULL` 保証を持つメソッドを定義する
  - ドキュメントコメントに「返却された `ProcessRun` の `pr_number` は必ず `Some(_)`」である旨を明記する
  - `impl std::future::Future<Output = Result<Option<ProcessRun>>> + Send` の形式に従う
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. SqliteProcessRunRepository に find_latest_with_pr_number を実装する
  - `find_latest` 実装を参考に、WHERE句に `AND pr_number IS NOT NULL` を追加した新メソッドを実装する
  - `spawn_blocking` + `Arc<Mutex<Connection>>` パターンに従う
  - クエリ結果のマッピングに既存の行マッピングロジックを再利用する
  - クエリ結果なしの場合は `Ok(None)` を返す
  - エラーは `.context("find_latest_with_pr_number failed")` でラップして伝播する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 3. observe_pr_for_type を find_latest_with_pr_number を使用するよう更新する
  - `process_repo.find_latest(...)` の呼び出しを `process_repo.find_latest_with_pr_number(...)` に変更する
  - `.and_then(|r| r.pr_number)` による通常のRustレベルのnullフィルタリングは削除する
  - `Some(run)` の場合でも `ProcessRun.pr_number` は `Option<u64>` のままなので、`find_latest_with_pr_number` の不変条件（`pr_number` は必ず `Some(_)`）に基づいて `u64` に落としてから `github.get_pr_status` を呼び出す
  - もし `pr_number` が `None` だった場合は、不変条件違反としてエラー化する（または到達不能として扱う）方針を実装に明記する
  - 既存の404エラーハンドリングロジックは変更しない
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 4. テストを追加して動作を検証する
  - [ ] 4.1 (P) observe_pr_for_type のユニットテストを追加する
    - モック `ProcessRunRepository` に `find_latest_with_pr_number` を実装する
    - `find_latest_with_pr_number` が `None` を返す場合 → `observe_pr_for_type` が `Ok(None)` を返すことを検証する
    - `find_latest_with_pr_number` が `Some(run)` (pr_number = Some(42)) を返す場合 → `github.get_pr_status(42)` が呼ばれることを検証する
    - 404エラーハンドリングが引き続き正しく動作することを検証する
    - _Requirements: 4.1, 4.2_

  - [ ] 4.2 (P) find_latest_with_pr_number のインテグレーションテストを追加する
    - インメモリSQLiteを使用する
    - 同一 `(issue_id, type_)` に対して `pr_number = NULL` と `pr_number = Some(n)` のレコードを混在させる
    - `find_latest_with_pr_number` がNULLレコードをスキップして最新の非NULLレコードを返すことを検証する
    - 全レコードの `pr_number` がNULLの場合に `Ok(None)` を返すことを検証する
    - _Requirements: 4.3_

  - [ ] 4.3 既存テストの回帰を確認する
    - `devbox run test` を実行して全テストがパスすることを確認する
    - `devbox run clippy` を実行して Clippy 警告がないことを確認する
    - _Requirements: 4.4_
