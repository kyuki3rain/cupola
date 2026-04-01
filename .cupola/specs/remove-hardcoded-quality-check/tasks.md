# Implementation Plan

## Task 1: 汎用品質チェック定数の追加
- [ ] 1.1 `GENERIC_QUALITY_CHECK_INSTRUCTION` 定数を `src/application/prompt.rs` のモジュールレベルに定義する
  - 定数値: `"commit 前に AGENTS.md / CLAUDE.md に記載された品質チェックを実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。"`
  - _Requirements: 1.2, 2.2, 3.2_

## Task 2: build_design_prompt の修正
- [ ] 2.1 `build_design_prompt` 内のステップ 6（`cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` の列挙）を `GENERIC_QUALITY_CHECK_INSTRUCTION` の埋め込みに置き換える
  - ステップ番号（6. 品質チェック → 7. commit）の整合性を維持する
  - _Requirements: 1.1, 1.2, 1.3_

## Task 3: build_implementation_prompt の修正
- [ ] 3.1 `build_implementation_prompt` 内の `quality_check_step` ブロックの `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` の列挙を `GENERIC_QUALITY_CHECK_INSTRUCTION` の埋め込みに置き換える
  - feature_name あり・なし両パターンで適用されることを確認する
  - _Requirements: 2.1, 2.2, 2.3_

## Task 4: build_fixing_prompt の修正
- [ ] 4.1 `build_fixing_prompt` 内のステップ 3（`cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` の列挙）を `GENERIC_QUALITY_CHECK_INSTRUCTION` の埋め込みに置き換える
  - ステップ番号（3. 品質チェック → 4. commit）の整合性を維持する
  - _Requirements: 3.1, 3.2, 3.3_

## Task 5: AGENTS.md の作成
- [ ] 5.1 リポジトリルート（`/AGENTS.md`）に AGENTS.md を新規作成する
  - commit 前に `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` を実行し、全てパスしてから commit すること、失敗した場合は修正して再チェックすること、を記載する
  - Claude Code はこのファイルを自動で読み込むため、プロンプト側の汎用指示と連携する
  - _Requirements: 1.2, 2.2, 3.2_

## Task 6: テストの更新
- [ ] 6.1 以下の旧テスト（cargo コマンド存在アサート）を削除する
  - `design_prompt_contains_quality_check`
  - `implementation_prompt_contains_quality_check`
  - `implementation_prompt_without_feature_name_contains_quality_check`
  - `fixing_prompt_contains_quality_check`
  - _Requirements: 4.1_
- [ ] 6.2 以下の新テストを追加する
  - `design_prompt_generic_quality_check`: design プロンプトが `cargo fmt` を含まず AGENTS.md 参照の汎用指示を含むことを検証
  - `implementation_prompt_generic_quality_check`: implementation プロンプト（feature_name あり）が同様であることを検証
  - `implementation_prompt_without_feature_name_generic_quality_check`: implementation プロンプト（feature_name なし）が同様であることを検証
  - `fixing_prompt_generic_quality_check`: fixing プロンプトが同様であることを検証
  - _Requirements: 4.2, 4.3_
