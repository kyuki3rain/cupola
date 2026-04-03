# Implementation Plan

## Task Format

---

- [x] 1. `Config::validate()` のバリデーションロジックを拡張する

- [x] 1.0 `log_dir` の必須化と空チェックを実装する
  - `Config.log_dir` の型を `Option<PathBuf>` から `PathBuf` に変更する
  - `default_with_repo` のデフォルト値を `PathBuf::from(".cupola/logs")` に設定する
  - `validate()` で `log_dir.as_os_str().is_empty()` のときエラーを返す処理を追加する
  - `config_loader.rs` の `into_config` で `log.dir` が未設定の場合 `".cupola/logs"` をデフォルト値として使用する
  - `app.rs` の daemon モード `cfg.log_dir.is_none()` チェックを削除する
  - `logging.rs` の `init_logging` 引数を `Option<&Path>` → `&Path`、戻り値を `Option<WorkerGuard>` → `WorkerGuard` に変更する
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [x] 1.1 文字列フィールドの空文字列チェックを追加する
  - `owner`、`repo`、`default_branch`、`language`、`model` の各フィールドに対して、空文字列の場合にエラーを返す処理を追加する
  - 評価順序: owner → repo → default_branch → language → model の順で評価し、最初に検出したエラーで早期リターンする
  - エラーメッセージはフィールド名を含む英語文字列とする（例: `"owner must not be empty"`）
  - 既存の `max_concurrent_sessions` チェックより前に配置する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 5.2, 6.1, 6.3_

- [x] 1.2 `polling_interval_secs` の絶対下限チェックを追加する
  - `polling_interval_secs` が 10 未満の場合にエラーを返す処理を追加する
  - 評価は文字列フィールドのチェック後、`stall_timeout_secs` のチェック前に行う
  - エラーメッセージに下限値（10）を含む英語文字列とする（例: `"polling_interval_secs must be at least 10"`）
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 5.2, 6.1, 6.2, 6.3_

- [x] 1.3 `stall_timeout_secs` の絶対下限チェックと相関チェックを追加する
  - `stall_timeout_secs` が 60 未満の場合にエラーを返す処理を追加する（絶対下限チェック）
  - 絶対下限チェックを通過した後にのみ、`stall_timeout_secs <= polling_interval_secs` の相関チェックを実施する
  - 絶対下限チェックのエラーメッセージに下限値（60）を含む英語文字列とする（例: `"stall_timeout_secs must be at least 60"`）
  - 相関チェックのエラーメッセージは `"stall_timeout_secs must be greater than polling_interval_secs"` とする
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3, 4.4, 5.2, 6.1, 6.2, 6.3_

---

- [x] 2. 追加バリデーションに対するユニットテストを実装する

- [x] 2.0 `log_dir` バリデーションに対するテストを追加する
  - `validate_rejects_empty_log_dir` — `log_dir` が空パス（`PathBuf::from("")`）のとき `Err` が返ることを確認
  - `validate_accepts_nonempty_log_dir` — `log_dir` が非空パスのとき OK が返ることを確認
  - `default_with_repo_log_dir_is_default_path` — デフォルト値が `".cupola/logs"` であることを確認
  - _Requirements: 6.1, 6.2_

- [x] 2.1 文字列フィールドの空文字列チェックに対するテストを追加する
  - `validate_rejects_empty_owner` — `owner` が空文字列のとき `Err` が返ることを確認
  - `validate_rejects_empty_repo` — `repo` が空文字列のとき `Err` が返ることを確認
  - `validate_rejects_empty_default_branch` — `default_branch` が空文字列のとき `Err` が返ることを確認
  - `validate_rejects_empty_language` — `language` が空文字列のとき `Err` が返ることを確認
  - `validate_rejects_empty_model` — `model` が空文字列のとき `Err` が返ることを確認
  - 既存テスト 3 件（`validate_rejects_zero_max_concurrent_sessions` 等）が引き続き通ることを確認
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 5.1, 5.2, 5.3_

- [x] 2.2 `polling_interval_secs` のバリデーションに対するテストを追加する
  - `validate_rejects_polling_below_minimum` — 値が 9 のとき `Err` が返ることを確認
  - `validate_rejects_polling_zero` — 値が 0 のとき `Err` が返ることを確認（境界値）
  - `validate_accepts_polling_at_minimum` — 値が 10 かつ他フィールドが有効なとき `Ok(())` が返ることを確認
  - `validate_accepts_polling_above_minimum` — 値が 11 以上のとき `Ok(())` が返ることを確認
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x] 2.3 `stall_timeout_secs` のバリデーションおよび相関チェックに対するテストを追加する
  - `validate_rejects_stall_below_minimum` — 値が 59 のとき `Err` が返ることを確認
  - `validate_rejects_stall_typical_misconfig` — 値が 30（秒/分取り違えの典型例）のとき `Err` が返ることを確認
  - `validate_accepts_stall_at_minimum_with_smaller_polling` — stall=60, polling=10 のとき `Ok(())` が返ることを確認
  - `validate_rejects_stall_equal_to_polling` — stall と polling が同値（共に有効な下限以上）のとき `Err` が返ることを確認（境界値）
  - `validate_rejects_stall_less_than_polling` — stall < polling（共に有効な下限以上）のとき `Err` が返ることを確認
  - `validate_accepts_stall_greater_than_polling` — stall > polling（共に有効な下限以上）のとき `Ok(())` が返ることを確認
  - `validate_skips_correlation_check_when_polling_too_small` — polling=5 のとき、相関チェックではなく polling の絶対下限エラーが返ることを確認
  - `validate_accepts_valid_default_config` — `Config::default_with_repo` が生成する設定が `Ok(())` を返すことを確認
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3, 4.4, 5.3_
