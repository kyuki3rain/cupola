# 要件定義書

## はじめに

Cupola の SQLite アダプター層（`sqlite_connection.rs`、`sqlite_execution_log_repository.rs`、`sqlite_issue_repository.rs`）では、`Arc<Mutex<Connection>>` の `lock()` 呼び出しと `tokio::task::spawn_blocking` の `await` 呼び出しに `.expect()` を使用している。mutex が毒化した場合、またはタスクがパニックした場合にプロセス全体がクラッシュするリスクがある。本フィーチャーは、これらの `.expect()` を `map_err` + `?` によるエラー伝搬に置き換え、呼び出し元が適切にエラーを処理できるようにする。

## 要件

### Requirement 1: mutex lock のエラー伝搬

**Objective:** アダプター開発者として、mutex の取得失敗を `Result` エラーとして受け取りたい。そうすることで、mutex 毒化時にプロセスがパニックせず、呼び出し元がエラーを適切にハンドリングできる。

#### 受け入れ条件

1. The SQLite Adapter shall 本番コード中の全ての `.expect("mutex poisoned")` を `map_err(|_| anyhow::anyhow!("failed to acquire database lock"))?` に置き換える。
2. When `mutex.lock()` が毒化エラーを返した場合, the SQLite Adapter shall `anyhow::Error` を返す（パニックしない）。
3. The SQLite Adapter shall 以下のファイルの本番コード全 11 箇所を対象とする：
   - `sqlite_connection.rs` の `init_schema()`（L42）
   - `sqlite_execution_log_repository.rs` の `record_start()`（L25）、`record_finish()`（L48）、`find_by_issue()`（L66）
   - `sqlite_issue_repository.rs` の `find_by_id()`（L26）、`find_by_issue_number()`（L46）、`find_active()`（L66）、`find_needing_process()`（L86）、`save()`（L107）、`update_state()`（L139）、`update()`（L155）、`reset_for_restart()`（L188）
4. The SQLite Adapter shall 各メソッドが既存のトレイトシグネチャ `-> Result<T>` を維持する（シグネチャ変更なし）。

### Requirement 2: spawn_blocking のエラー伝搬

**Objective:** アダプター開発者として、`spawn_blocking` タスクの失敗を `Result` エラーとして受け取りたい。そうすることで、タスクのパニック時にプロセスがクラッシュせず、エラーが上位レイヤーに伝搬される。

#### 受け入れ条件

1. The SQLite Adapter shall 本番コード中の全ての `.await.expect("spawn_blocking panicked")` を `.await.map_err(|e| anyhow::anyhow!("spawn_blocking task failed: {e}"))?` に置き換える。
2. If `spawn_blocking` タスクがパニックした場合, the SQLite Adapter shall `JoinError` を `anyhow::Error` に変換して返す（プロセスをパニックさせない）。
3. The SQLite Adapter shall 置き換え後もメソッドの戻り値型 `Result<T>` を維持する。

### Requirement 3: テストコードの保護

**Objective:** テスト担当者として、テストコード内の `.expect()` は変更されないことを確認したい。そうすることで、テストの可読性と意図が損なわれない。

#### 受け入れ条件

1. The SQLite Adapter shall `sqlite_connection.rs` の L124、L154、L188 のテストコード内 `.expect()` を変更しない。
2. The SQLite Adapter shall `#[cfg(test)]` ブロック内の `.expect()` を一切変更しない。

### Requirement 4: コード品質の維持

**Objective:** 開発者として、変更後もプロジェクトの品質基準を満たすことを確認したい。そうすることで、CI が継続して通過し、リグレッションが発生しない。

#### 受け入れ条件

1. When 本変更が適用された場合, the SQLite Adapter shall `cargo test` が全テストをパスする。
2. When 本変更が適用された場合, the SQLite Adapter shall `cargo clippy -- -D warnings` が警告・エラーなしで通過する。
3. The SQLite Adapter shall `cargo fmt` で整形された状態を維持する。
