# Implementation Plan

## bugfix-graphql-session-pid

---

- [ ] 1. GraphQL クライアントの variables 化
- [ ] 1.1 (P) クエリ文字列をstaticな文字列に変更し、パラメータをvariablesに分離する
  - `list_unresolved_threads` 内のページネーションループから `format!` による文字列補間を除去する
  - `owner`・`repo`・`pr_number`・`after`（nullable）を `serde_json::json!` で variables オブジェクトとして構築する
  - `execute_query` の代わりに `execute_raw` を直接呼び、`check_graphql_errors` を呼び出してエラー処理を行う
  - ページネーションの `after` カーソルは `Option<String>` を `Value::String` または `Value::Null` に変換して渡す
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 1.2 (P) variables 形式での動作を検証するテストを追加する
  - `list_unresolved_threads` が送信するペイロードの `variables` フィールドに owner/repo/pr_number が含まれることを確認するテストを追加する
  - クエリ文字列に owner/repo/pr_number のリテラル値が含まれていないことを確認するテストを追加する
  - ページネーションカーソルが null および String として variables に含まれることを確認するテストを追加する
  - _Requirements: 1.4_

- [ ] 2. SessionManager での旧セッション kill 処理の追加
- [ ] 2.1 (P) 同一 issue_id への二重登録時に旧プロセスを終了させる
  - `register()` の冒頭で `sessions.remove(&issue_id)` を呼び、旧エントリを取得する
  - 旧エントリが存在する場合は `old_entry.child.kill()` を呼び、`let _ =` でエラーを無視する
  - その後、既存の stdout/stderr スレッド起動・`SessionEntry` 構築・`sessions.insert` ロジックを変更せずに継続する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 2.2 (P) 旧プロセスが kill されることをテストで検証する
  - 同一 issue_id に対して `spawn_sleep` で作成した Child を `register()` してから、別の Child で再度 `register()` するテストを追加する
  - 旧プロセスの PID が一定時間内に終了していること（`try_wait` が `Some` を返すこと）を確認する
  - _Requirements: 2.4_

- [ ] 3. PID ファイルの TOCTOU 修正
- [ ] 3.1 (P) `PidFileError` に `AlreadyExists` バリアントを追加する
  - `application/port/pid_file.rs` の `PidFileError` enum に `AlreadyExists` バリアントを追加する
  - `thiserror` の `#[error]` アトリビュートで適切なエラーメッセージを設定する
  - 既存の `PidFileError` に対する `match` 式がコンパイルエラーになる箇所をすべて更新する
  - _Requirements: 3.2_

- [ ] 3.2 (P) `write_pid` を排他的ファイル作成に変更する
  - `std::fs::write` の代わりに `OpenOptions::new().write(true).create_new(true).open(path)` でファイルを開く
  - `ErrorKind::AlreadyExists` の場合は `PidFileError::AlreadyExists` を返す
  - その他のエラーは `PidFileError::Write` でラップして返す
  - ファイルオープン成功後は `write_all` でPIDを書き込む
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 3.3 (P) PID ファイル競合および正常作成のテストを追加する
  - `write_pid` を2回連続で呼んだ場合に `PidFileError::AlreadyExists` が返ることを確認するテストを追加する
  - `delete_pid` 後に `write_pid` が成功することを確認するテストを追加する
  - 既存テストが引き続きパスすることを確認する
  - _Requirements: 3.5_

- [ ] 4. `bootstrap/app.rs` での `AlreadyExists` エラー処理の追加
  - デーモン起動フローで `write_pid` が `PidFileError::AlreadyExists` を返した場合に「cupola is already running」メッセージを表示して早期終了する処理を追加する
  - _Requirements: 3.2_
