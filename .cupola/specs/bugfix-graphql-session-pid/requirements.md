# Requirements Document

## Project Description (Input)
GraphQL variables化・session上書き防止・PIDファイルTOCTOU修正の3件のバグ修正

## はじめに

本仕様は、Cupolaシステムにおける3件のMediumレベルのバグ修正を対象とする。

1. **M1**: `github_graphql_client.rs` の `list_unresolved_threads` でGraphQLクエリに直接文字列補間していた箇所をvariables化する
2. **M2**: `session_manager.rs` の `register()` にて既存セッションを上書きした際に旧Childプロセスが孤児化する問題を修正する
3. **M3**: `pid_file_manager.rs` / `app.rs` のPIDファイル書き込みにおけるTOCTOUレース条件を排他的ファイル作成で修正する

## Requirements

### Requirement 1: GraphQL クエリの variables 化

**Objective:** 開発者として、GraphQLクエリへのパラメータ埋め込みをvariables経由に統一することで、インジェクションリスクを排除し、コードの一貫性を高めたい。

#### Acceptance Criteria

1. When `list_unresolved_threads` が呼び出されたとき、the GitHub GraphQL Client shall `owner`・`repo`・`pr_number` を `format!` による文字列補間ではなく、GraphQL variables オブジェクトとして送信する
2. The GitHub GraphQL Client shall `reply_to_thread` および `resolve_thread` と同じ variables 形式（`serde_json::json!` によるオブジェクト構築）を使用する
3. When GraphQL クエリが送信されるとき、the GitHub GraphQL Client shall クエリ文字列に owner・repo・pr_number のリテラル値を含まない
4. The GitHub GraphQL Client shall 既存の `list_unresolved_threads` に対するテストが variables 形式でも正常にパスする

### Requirement 2: SessionManager における既存セッションの上書き防止

**Objective:** 開発者として、同一 issue_id に対するセッション二重登録時に旧プロセスを適切に終了させることで、孤児プロセスの発生を防止したい。

#### Acceptance Criteria

1. When `register()` が呼び出されたとき and 同一 `issue_id` のセッションが既に存在するとき、the Session Manager shall 旧セッションの `Child` プロセスに対して `kill()` を呼び出してから新しいセッションを登録する
2. When `register()` で旧プロセスの `kill()` が失敗したとき、the Session Manager shall エラーを無視して新しいセッションの登録処理を継続する
3. If 同一 `issue_id` のセッションが存在しないとき、the Session Manager shall 既存の登録処理と同等の動作を行う
4. When 同一 issue_id に対して二重 `register()` が呼ばれたとき、the Session Manager shall 旧プロセスが kill されることをテストで検証できる

### Requirement 3: PID ファイル TOCTOU レース条件の修正

**Objective:** 開発者として、PIDファイルの存在確認と書き込みをアトミックに行うことで、デーモンの二重起動を確実に防止したい。

#### Acceptance Criteria

1. When `write_pid()` が呼び出されたとき、the PID File Manager shall `OpenOptions` + `create_new(true)`（O_EXCL相当）を使用してPIDファイルを排他的に作成する
2. If PIDファイルが既に存在するとき、the PID File Manager shall `write_pid()` をエラーで失敗させ、二重起動を防止する
3. When stale PIDファイルのクリーンアップ後に `write_pid()` が呼び出されるとき、the PID File Manager shall 正常にPIDファイルを作成できる
4. The PID File Manager shall `check_and_clean_pid_file` と `write_pid` の間に競合状態（TOCTOU）が発生しない設計を保証する
5. When PIDファイルが既に存在する状態で `write_pid()` を呼ぶとき、the PID File Manager shall エラーを返すことをテストで検証できる
