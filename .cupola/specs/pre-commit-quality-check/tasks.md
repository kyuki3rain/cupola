# Implementation Plan

## Task Format Template

- [ ] 1. 設計エージェントプロンプトに品質チェック手順を追加する
- [ ] 1.1 `build_design_prompt` のプロンプト文字列に品質チェックセクションを追加する
  - commitの直前（手順5と手順6の間）に独立した手順番号として品質チェックを挿入する
  - `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` の3コマンドを順番に記載する
  - いずれかが失敗した場合は修正してから再度チェックすること、という指示を含める
  - 既存の手順6（git add / git commit / git push）を手順7に繰り下げる
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. 実装エージェントプロンプトに品質チェック手順を追加する
- [ ] 2.1 `build_implementation_prompt` のプロンプト文字列に品質チェックセクションを追加する
  - 実装ステップの後、pushステップの直前に品質チェック手順を挿入する
  - `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` の3コマンドを順番に記載する
  - いずれかが失敗した場合は修正してから再度チェックすること、という指示を含める
  - `feature_name` の有無に応じて手順番号が変わる変数（`quality_check_step`）を導入し、pushステップの番号（`push_step`）を更新する
  - `feature_name` ありの場合: 実装=1, 品質チェック=2, push=3
  - `feature_name` なしの場合: spec確認=1, 実装=2, 品質チェック=3, push=4
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ] 3. レビュー対応エージェントプロンプトに品質チェック手順を追加する
- [ ] 3.1 `build_fixing_prompt` のプロンプト文字列に品質チェックセクションを追加する
  - 各スレッド対応（手順2）の後、commitステップ（手順3）の直前に品質チェックを挿入する
  - `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` の3コマンドを順番に記載する
  - いずれかが失敗した場合は修正してから再度チェックすること、という指示を含める
  - 既存の手順3（git add -A / git commit / git push）を手順4に繰り下げる
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 4. 品質チェック手順の存在を検証するユニットテストを追加する
- [ ] 4.1 (P) 設計プロンプトの品質チェック内容を検証するテストを追加する
  - `build_design_prompt` のプロンプトに `cargo fmt` が含まれることを確認するテストを追加する
  - `build_design_prompt` のプロンプトに `cargo clippy -- -D warnings` が含まれることを確認するテストを追加する
  - `build_design_prompt` のプロンプトに `cargo test` が含まれることを確認するテストを追加する
  - _Requirements: 1.1, 4.3_

- [ ] 4.2 (P) 実装プロンプトの品質チェック内容を検証するテストを追加する
  - `feature_name` ありの場合に品質チェック文言が含まれることを確認するテストを追加する
  - `feature_name` なしの場合に品質チェック文言が含まれることを確認するテストを追加する
  - _Requirements: 2.1, 2.4, 4.3_

- [ ] 4.3 (P) レビュー対応プロンプトの品質チェック内容を検証するテストを追加する
  - `build_fixing_prompt` のプロンプトに `cargo fmt` が含まれることを確認するテストを追加する
  - `build_fixing_prompt` のプロンプトに `cargo clippy -- -D warnings` が含まれることを確認するテストを追加する
  - `build_fixing_prompt` のプロンプトに `cargo test` が含まれることを確認するテストを追加する
  - _Requirements: 3.1, 4.3_

- [ ] 5. 既存テストとの整合性を確認する
- [ ] 5.1 既存ユニットテストがすべてパスすることを確認する
  - `cargo test` を実行してすべてのテストがパスすることを確認する
  - 既存テスト（`design_running_returns_pr_creation_schema` 等）が引き続きパスすることを確認する
  - `output_schema` の種別が変更されていないことを確認する
  - _Requirements: 4.1, 4.2_
