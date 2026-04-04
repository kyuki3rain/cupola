# Research & Design Decisions

---
**Purpose**: 設計調査の記録および設計判断の根拠を保持する。

---

## Summary
- **Feature**: `bugfix-graphql-session-pid`
- **Discovery Scope**: Extension（既存システムへの修正）
- **Key Findings**:
  - GraphQLのvariables化は `reply_to_thread` / `resolve_thread` の既存パターンを踏襲するが、ページネーションカーソル（`after`）がオプション引数であるため nullable variable として扱う必要がある
  - `SessionManager.register()` の孤児化問題は `HashMap::insert` の戻り値（旧エントリ）を利用することで最小変更で修正可能
  - `PidFileError` に `AlreadyExists` バリアントを追加することで、TOCTOU修正後の呼び出し元が「ファイルが既に存在する」ケースを明示的に処理できる

## Research Log

### GraphQL Variables パターン調査

- **Context**: `list_unresolved_threads` が `format!` で owner/repo/pr_number を埋め込んでいるが、同ファイルの他メソッドは variables を使用している
- **Sources Consulted**: 同ファイル `reply_to_thread`・`resolve_thread` の実装（行89-128）
- **Findings**:
  - `reply_to_thread` と `resolve_thread` は `json!({ "query": query, "variables": variables })` 形式で `execute_raw` を直接呼ぶ
  - `execute_query` は現在 variables 未対応（query のみのペイロードを組み立てる）
  - ページネーション用のカーソル `after` は省略可能なため、`$after: String` (nullable) として宣言し `null` を渡す設計が標準的
- **Implications**: `execute_query` を変更するか、`execute_raw` を直接呼ぶ形に変更する必要がある

### SessionManager 孤児プロセス問題

- **Context**: `HashMap::insert` が旧値を返すが現在はその戻り値を捨てており、旧 `Child` がドロップされても kill されない
- **Findings**:
  - `Child::drop` は Unix では何もしない（プロセスは引き続き動作する）
  - `HashMap::insert` の戻り値 `Option<SessionEntry>` を受け取り、`entry.child.kill()` を呼ぶだけで修正可能
  - `kill()` の失敗は無視して新セッションの登録を続行する（`let _ = old.child.kill()`）
- **Implications**: `register` メソッドの先頭数行のみの変更で対応可能。`mut` の付け方に注意

### PID ファイル TOCTOU レース条件

- **Context**: `check_and_clean_pid_file` で既存PIDを確認・削除した後、`write_pid` でファイルを作成するまでの間に別プロセスが先に書き込む可能性がある
- **Sources Consulted**: `std::fs::OpenOptions` ドキュメント、POSIX O_EXCL セマンティクス
- **Findings**:
  - `OpenOptions::new().write(true).create_new(true)` は O_CREAT | O_EXCL に相当し、ファイルが既存であれば `ErrorKind::AlreadyExists` で失敗する
  - stale PID クリーンアップ後でも `create_new(true)` を使えば二重起動を確実に防止できる
  - `PidFileError::Write` で AlreadyExists を包んでしまうと呼び出し元が区別できないため、専用バリアント `AlreadyExists` の追加が望ましい
- **Implications**: `PidFilePort` トレイトの `PidFileError` に `AlreadyExists` を追加し、`PidFileManager::write_pid` の実装を `OpenOptions` + `create_new(true)` + `write_all` に変更する

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|-----------|------|------|------------|------|
| execute_query に variables 引数追加 | 既存 `execute_query` に `variables: Option<Value>` を追加 | 統一インターフェース | signature 変更が既存呼び出し元に影響 | 既存呼び出し元が1箇所のみであれば採用可 |
| execute_raw を直接呼ぶ | `list_unresolved_threads` が `execute_raw` を直接呼ぶ | 変更最小 | `execute_query` との二重管理 | `reply_to_thread`/`resolve_thread` と同パターン |
| OpenOptions + create_new | O_EXCL で排他的作成 | OSレベルのアトミック性 | なし | POSIX標準、Rustの std でサポート |

## Design Decisions

### Decision: `execute_raw` を直接呼ぶ方式を採用

- **Context**: `list_unresolved_threads` を variables 対応させる際の内部API選択
- **Alternatives Considered**:
  1. `execute_query` に `variables: Option<Value>` 引数を追加
  2. `execute_raw` を直接呼ぶ（`reply_to_thread` と同パターン）
- **Selected Approach**: `execute_raw` を直接呼ぶ形に変更し、`execute_query` はシグネチャを変更しない
- **Rationale**: 既存の `reply_to_thread`・`resolve_thread` と同一パターンで一貫性が高い。`execute_query` の呼び出し元への影響がない
- **Trade-offs**: `execute_query` は引き続き variables 非対応のままだが、現在の用途には問題ない
- **Follow-up**: ページネーションループで `after` を nullable variable として渡す実装の確認

### Decision: `PidFileError::AlreadyExists` バリアント追加

- **Context**: `create_new(true)` でファイルが既存の場合にエラーを返す際の型設計
- **Alternatives Considered**:
  1. `PidFileError::Write` に AlreadyExists をラップして返す
  2. 専用バリアント `AlreadyExists` を追加する
- **Selected Approach**: `PidFileError::AlreadyExists` を新規追加
- **Rationale**: 呼び出し元（`app.rs`）がTOCTOU競合とその他書き込みエラーを区別してメッセージを出力できる
- **Trade-offs**: トレイトのエラー型変更が必要だが、後方互換性は破壊しない（非exhaustive ではないため、テストの match 文を更新する必要あり）

## Risks & Mitigations

- GraphQL nullable variable のクエリ文字列で `$after: String` と宣言する必要があり、`String!` (non-null) と混同しないよう注意 — GitHub GraphQL API スキーマを確認して nullable を明示する
- `register()` の既存テストでは `insert` の上書き動作を直接テストしていないため、新テストを追加して旧プロセスがkillされることを確認する
- `PidFileError` の match が非網羅的になる可能性 — コンパイラが警告を出すため実装時に発覚できる

## References

- `src/adapter/outbound/github_graphql_client.rs:89-128` — `reply_to_thread`/`resolve_thread` の variables パターン
- `src/application/port/pid_file.rs` — `PidFileError` 定義
- Rust std `std::fs::OpenOptions::create_new` — O_EXCL相当の排他的ファイル作成
