# Implementation Plan

- [ ] 1. 共通ヘルパー関数を抽出する
- [ ] 1.1 マージコンフリクト解消セクション生成を `build_conflict_section` として切り出す
  - `build_fixing_prompt` 内の `conflict_section` 生成ロジックを `fn build_conflict_section(has_merge_conflict: bool, default_branch: &str) -> String` として抽出する
  - 既存ロジックを変更せずそのまま移植する
  - _Requirements: 3.1_

- [ ] 1.2 (P) causes から指示文リストを生成する `build_instructions_text` を切り出す
  - `build_fixing_prompt` 内の `instructions_text` 生成ロジックを `fn build_instructions_text(causes: &[FixingProblemKind]) -> String` として抽出する
  - 既存ロジックを変更せずそのまま移植する
  - _Requirements: 3.2, 3.3_

- [ ] 1.3 (P) output-schema セクション生成を `build_output_section` として切り出す
  - `build_fixing_prompt` 内の `output_section` 生成ロジックを `fn build_output_section(causes: &[FixingProblemKind], language: &str) -> String` として抽出する
  - 既存ロジックを変更せずそのまま移植する
  - _Requirements: 3.2, 3.3_

- [ ] 2. `build_design_fixing_prompt` を実装する
- [ ] 2.1 DesignFixing 専用プロンプト関数を新規作成する
  - `fn build_design_fixing_prompt(issue_number: u64, pr_number: u64, language: &str, causes: &[FixingProblemKind], has_merge_conflict: bool, default_branch: &str) -> String` を追加する
  - プロンプト文言を設計ドキュメント修正向けに変更する（「Apply the necessary fixes to the design documents」相当）
  - コミット指示を `git commit -m "docs: address requested changes"` とする
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` を品質チェック指示として使用する
  - タスク 1 で抽出したヘルパーを呼び出してセクションを組み立てる
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [ ] 3. `build_implementation_fixing_prompt` を実装する
- [ ] 3.1 ImplementationFixing 専用プロンプト関数を新規作成する
  - `fn build_implementation_fixing_prompt(issue_number: u64, pr_number: u64, language: &str, causes: &[FixingProblemKind], has_merge_conflict: bool, default_branch: &str) -> String` を追加する
  - 現行 `build_fixing_prompt` の文言・構造を維持する（コミット指示 `fix:` を含む）
  - タスク 1 で抽出したヘルパーを呼び出してセクションを組み立てる
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

- [ ] 4. `build_session_config` の呼び出し先を切り替える
- [ ] 4.1 `State::DesignFixing` の分岐で `build_design_fixing_prompt` を呼ぶよう変更する
  - `build_session_config` 内の `State::DesignFixing` 分岐を `build_design_fixing_prompt` 呼び出しに変更する
  - `State::ImplementationFixing` 分岐を `build_implementation_fixing_prompt` 呼び出しに変更する
  - 旧 `build_fixing_prompt` を削除する
  - _Requirements: 1.1, 2.1, 3.5_

- [ ] 5. テストを整備する
- [ ] 5.1 `DesignFixing` 専用テストを追加する
  - `design_fixing_prompt_contains_docs_prefix` — `DesignFixing` のプロンプトに `docs:` コミット指示が含まれることを検証する
  - `design_fixing_prompt_does_not_contain_code_wording` — `DesignFixing` のプロンプトに「fix to the code」等のコード向け文言が含まれないことを検証する
  - _Requirements: 4.1, 1.2, 1.3_

- [ ] 5.2 (P) `ImplementationFixing` 専用テストを追加する
  - `implementation_fixing_prompt_contains_fix_prefix` — `ImplementationFixing` のプロンプトに `fix:` コミット指示が含まれることを検証する
  - `implementation_fixing_prompt_contains_code_wording` — `ImplementationFixing` のプロンプトにコード修正向けの文言が含まれることを検証する
  - _Requirements: 4.2, 2.2, 2.3_

- [ ] 5.3 (P) 既存の `fixing_prompt_*` テスト群を `ImplementationFixing` 状態でも実行されるよう拡張する
  - `fixing_prompt_review_comments_only` / `fixing_prompt_ci_failure_only` / `fixing_prompt_conflict_only` / `fixing_prompt_all_causes` / `fixing_prompt_no_git_add_all` / `fixing_prompt_has_merge_conflict_*` について、`State::ImplementationFixing` でも同等のアサーションが通ることを確認するテストを追加する
  - _Requirements: 4.3, 3.1, 3.2, 3.3, 3.4_

- [ ] 5.4 (P) `DesignFixing` の causes ハンドリングテストを追加する
  - `design_fixing_review_comments` — `ReviewComments` cause で `review_threads.json` 参照が含まれることを検証する
  - `design_fixing_ci_failure` — `CiFailure` cause で `ci_errors.txt` 参照が含まれることを検証する
  - `design_fixing_no_git_add_all` — `git add -A` が含まれないことを検証する
  - _Requirements: 4.3, 1.5, 1.6, 3.4_
