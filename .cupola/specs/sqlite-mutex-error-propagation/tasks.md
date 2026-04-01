# 実装タスク

## Task Format Template

- [ ] 1. DB接続ラッパーの mutex lock エラー伝搬
- [ ] 1.1 (P) `init_schema` 内の mutex lock を安全化
  - `sqlite_connection.rs` の `init_schema()` 内にある `.expect("mutex poisoned")` を `map_err(|_| anyhow::anyhow!("failed to acquire database lock"))?` に置き換える
  - `init_schema()` の戻り値型 `-> anyhow::Result<()>` はそのまま維持する
  - テストコード（`#[cfg(test)]` ブロック）内の `.expect()` は一切変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 3.1, 3.2_

- [ ] 2. 実行ログリポジトリの mutex lock および spawn_blocking エラー伝搬
- [ ] 2.1 (P) `record_start`、`record_finish`、`find_by_issue` の mutex lock を安全化
  - `sqlite_execution_log_repository.rs` の `record_start()`（L25）、`record_finish()`（L48）、`find_by_issue()`（L66）にある `.expect("mutex poisoned")` を `map_err(|_| anyhow::anyhow!("failed to acquire database lock"))?` に置き換える
  - 各メソッドの `ExecutionLogRepository` トレイトシグネチャは変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2.2 (P) `record_start`、`record_finish`、`find_by_issue` の spawn_blocking await を安全化
  - 上記3メソッドの末尾にある `.await.expect("spawn_blocking panicked")` を `.await.map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?` に置き換える
  - 各メソッドの戻り値型 `-> anyhow::Result<T>` を維持する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 3. Issue リポジトリの mutex lock および spawn_blocking エラー伝搬
- [ ] 3.1 (P) 全8メソッドの mutex lock を安全化
  - `sqlite_issue_repository.rs` の `find_by_id()`（L26）、`find_by_issue_number()`（L46）、`find_active()`（L66）、`find_needing_process()`（L86）、`save()`（L107）、`update_state()`（L139）、`update()`（L155）、`reset_for_restart()`（L188）にある `.expect("mutex poisoned")` を `map_err(|_| anyhow::anyhow!("failed to acquire database lock"))?` に置き換える
  - `IssueRepository` トレイトシグネチャは変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 3.2 (P) 全8メソッドの spawn_blocking await を安全化
  - 上記8メソッドの末尾にある `.await.expect("spawn_blocking panicked")` を `.await.map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?` に置き換える
  - 各メソッドの戻り値型 `-> anyhow::Result<T>` を維持する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 4. 品質確認
- [ ] 4.1 静的解析とテストの実行
  - `RUSTFLAGS="-D warnings" cargo clippy --all-targets` を実行し、警告・エラーが0件であることを確認する
  - `cargo fmt -- --check` を実行し、フォーマット違反がないことを確認する
  - `cargo test --lib -- --test-threads=1` を実行し、全テストがパスすることを確認する
  - テストコード内の `.expect()` が変更されていないことを確認する（`sqlite_connection.rs` L124, L154, L188）
  - _Requirements: 3.1, 3.2, 4.1, 4.2, 4.3_
