# Implementation Plan

- [ ] 1. (P) prompt.rs の build_session_config をエラー伝播に対応させる
- [ ] 1.1 (P) build_session_config の戻り値型を Result に変更し、pr_number 欠如時にエラーを返す
  - DesignFixing および ImplementationFixing 状態で `pr_number` が `None` の場合、`anyhow::Context` を使って `Err("fixing state requires pr_number in DB")` を返す
  - 戻り値型を `SessionConfig` から `Result<SessionConfig, anyhow::Error>` に変更する
  - 他の状態では従来通り `Ok(SessionConfig)` を返す
  - `session_manager.rs` の `expect("wait after kill")` は変更対象外であることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 4.1, 4.2_

- [ ] 2. step7_spawn_processes でのエラースキップ処理を追加する
- [ ] 2.1 step7_spawn_processes で build_session_config のエラー時に該当 Issue をスキップする
  - Task 1 完了後に着手すること（`build_session_config` の戻り値型変更が前提）
  - `build_session_config` が `Err` を返した場合、`tracing::warn!` でエラーを記録する
  - エラーが発生した Issue のみスキップし、他の Issue の spawn 処理を継続する（`continue` パターン）
  - 既存のループ内エラースキップパターンと一致する形で実装する
  - _Requirements: 1.4, 1.5_

- [ ] 3. (P) transition_use_case.rs の expect() をエラー伝播に変更する
- [ ] 3.1 (P) reset_for_restart 後の find_by_id で None の場合にエラーを伝播させる
  - `reset_for_restart` 直後の `find_by_id` 呼び出しで `None` が返った場合、`context("issue not found after reset_for_restart")?` でエラーとして伝播させる
  - `step6_apply_events` 内の既存 `if let Err(e)` ハンドリングはそのまま維持し、変更しない
  - `session_manager.rs` の `expect("wait after kill")` は変更対象外であることを確認する
  - _Requirements: 2.1, 2.2, 2.3, 4.1, 4.2_

- [ ] 4. (P) clippy lint によるリグレッション防止を設定する
- [ ] 4.1 (P) Cargo.toml の clippy lint 設定に expect_used = "deny" を追加する
  - 既存の `[lints.clippy]` セクションに `expect_used = "deny"` を追記する
  - `all = "warn"` はすでに設定済みのため変更不要
  - _Requirements: 3.1, 3.2_

- [ ] 4.2 (P) src/lib.rs でテストコードの expect_used lint を許可する
  - `#![cfg_attr(test, allow(clippy::expect_used))]` をクレートレベルアトリビュートとして `src/lib.rs` 先頭付近に追記する
  - `#[cfg(test)]` ブロック内の `expect()` がエラーにならないことが目的
  - _Requirements: 3.3, 3.5_

- [ ] 5. テスト・lint 検証を実施する
- [ ] 5.1 build_session_config の各状態に対するユニットテストを追加する
  - DesignFixing 状態かつ `pr_number = None` のとき `Err` が返ることを確認するテストを追加
  - ImplementationFixing 状態かつ `pr_number = None` のとき `Err` が返ることを確認するテストを追加
  - それ以外の状態かつ `pr_number = None` のとき `Ok` が返ることを確認するテストを追加
  - _Requirements: 1.2, 1.3_

- [ ] 5.2 transition_use_case の reset 後エラー伝播に関する統合テストを確認する
  - `reset_for_restart` 後に `find_by_id` が `None` を返す場合に `Err` が伝播されることを確認する
  - `step6_apply_events` の既存エラーハンドリングで正しくキャッチされることを確認する
  - _Requirements: 2.1, 2.2_

- [ ] 5.3 cargo clippy で全エラーなしを確認する
  - `cargo clippy -- -D warnings` を実行し、エラーが0件であることを確認する
  - テストコード内の `expect()` が `deny` エラーにならないことを確認する（`cfg_attr` の効果確認）
  - `cargo test` を実行し、既存テストがすべてパスすることを確認する
  - _Requirements: 3.4, 3.5_
