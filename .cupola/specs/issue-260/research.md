# Research & Design Decisions

---
**Purpose**: Mutex毒化処理の修正に向けた調査記録・設計判断の根拠。

---

## Summary

- **Feature**: `issue-260` — SQLiteリポジトリにおけるMutexロック毒化の致命的エラー処理
- **Discovery Scope**: Extension（既存アダプター層の修正）
- **Key Findings**:
  - RustのMutex毒化（PoisonError）は前スレッドがクリティカルセクション内でパニックした場合にのみ発生する。回復不可能な状態である。
  - 現状の `map_err(|e| anyhow::anyhow!(...))` はエラーを `?` で伝播させ、呼び出しコードが "retry" や "log and continue" できてしまう誤った設計である。
  - `SqliteConnection` にヘルパーメソッド `conn_lock()` を追加することで、4ファイル・約30箇所のロック取得コードを一元化できる。
  - プロジェクトの Clippy 設定では `expect_used = "deny"` が適用されているため、`.expect()` は使用不可。正しいパターンは `.unwrap_or_else(|e| panic!(...))` である。

## Research Log

### Mutex毒化（PoisonError）のRustセマンティクス

- **Context**: `.lock()` が返す `PoisonError` の意味と適切な処理方法を確認する。
- **Findings**:
  - `std::sync::Mutex::lock()` は `Result<MutexGuard<T>, PoisonError<MutexGuard<T>>>` を返す。
  - `PoisonError` はあるスレッドが `MutexGuard` を保持したままパニックした場合にのみ発生する。
  - 毒化後のすべての `.lock()` 呼び出しは `Err(PoisonError)` を返す（`.into_inner()` で強制取得する回避策は存在するが、データ整合性が保証されない）。
  - Rustの標準ライブラリのドキュメントでは、毒化ロックは「前スレッドが不整合な状態でデータを残した可能性がある」ことを示すと明記されている。
- **Implications**: SQLiteのMutexが毒化している場合、接続オブジェクトの状態が不整合である可能性がある。即時 `panic!` が正しい対処である。

### 既存コードの毒化処理パターン

- **Context**: 影響範囲とコードパターンを特定するためにコードベースを調査した。
- **Findings**:
  - `sqlite_connection.rs`: `init_schema()` 内で1箇所
  - `sqlite_issue_repository.rs`: 全メソッドで計10箇所以上
  - `sqlite_process_run_repository.rs`: 全メソッドで計15箇所以上
  - `sqlite_execution_log_repository.rs`: 全メソッドで計3箇所
  - すべてのパターンが `.lock().map_err(|e| anyhow::anyhow!("failed to acquire database lock: {e}"))` で統一されている。
  - テストコードでは `.lock().expect("mutex")` が使われており、テストは別途対応が不要。
- **Implications**: ヘルパーメソッドによる集約が最も効率的な修正手段である。

### Clippy制約の確認

- **Context**: `Cargo.toml` の `[lints.clippy]` 設定を確認した。
- **Findings**:
  - `all = { level = "warn", priority = -1 }` — 全Clippy警告をwarnに設定。
  - `expect_used = "deny"` — `.expect()` の使用を禁止。
  - `unwrap_used` は明示的に deny されていない（warn レベル）。
- **Implications**: `.expect("...")` は使用不可。`.unwrap_or_else(|e| panic!(...))` が適切なパターン。

## Architecture Pattern Evaluation

| Option | 説明 | 強み | リスク/制約 | 備考 |
|--------|------|------|-------------|------|
| 各呼び出しサイトを個別修正 | 30箇所すべてで `map_err` を `unwrap_or_else(panic)` に置換 | シンプル | 修正漏れのリスク、コードの重複 | 小規模修正向き |
| `conn_lock()` ヘルパーメソッドを追加 | `SqliteConnection` に安全なロック取得メソッドを集約 | DRY、一元化、テストしやすい | `SqliteConnection` のAPI拡張 | **採用** |

## Design Decisions

### Decision: `conn_lock()` ヘルパーメソッドの導入

- **Context**: 約30箇所に散在する同一パターンを安全に修正する方法を選択する。
- **Alternatives Considered**:
  1. 各呼び出しサイトを個別修正 — パターンを直接置換する
  2. `conn_lock()` ヘルパーメソッドを `SqliteConnection` に追加する
- **Selected Approach**: `SqliteConnection::conn_lock()` メソッドを追加し、`MutexGuard<Connection>` を返す。内部で `unwrap_or_else(|e| panic!(...))` を使い、毒化時に即時パニック。
- **Rationale**: 1箇所の変更で全リポジトリに適用でき、修正漏れが生じない。パニックメッセージとコメントも1箇所で管理できる。既存の `conn()` メソッド（`Arc<Mutex<Connection>>` を返す）は引き続き公開してもよいが、リポジトリからは `conn_lock()` を使うよう統一する。
- **Trade-offs**: `SqliteConnection` のAPIが若干増えるが、内部実装詳細の隠蔽として適切。
- **Follow-up**: テストコードにおける毒化Mutexの再現方法は `std::thread::spawn` + `panic` + `join` のパターンで対応可能。

## Risks & Mitigations

- **リスク**: パニックはスレッドを強制終了させるため、tokio の spawn_blocking タスクがパニックすると JoinError が上位に伝播する。
  - **緩和策**: 現状でも Mutex 毒化後の操作はデータ整合性が保証されないため、パニックによる即時終了は正しい動作。JoinError は呼び出し元でそのままエラーとして扱われ、pollingループがリトライしないよう上位ロジックで対処済み（または今後対処すべき事項）。
- **リスク**: テストでの毒化Mutex再現が複雑になる可能性がある。
  - **緩和策**: `std::thread::spawn(|| { let _g = mutex.lock(); panic!("poison"); }).join()` のパターンで確実に毒化できる。

## References

- [std::sync::Mutex - Rust Standard Library](https://doc.rust-lang.org/std/sync/struct.Mutex.html) — PoisonError のセマンティクス
- [clippy::expect_used lint](https://rust-lang.github.io/rust-clippy/master/index.html#expect_used) — 本プロジェクトで deny 設定済み
