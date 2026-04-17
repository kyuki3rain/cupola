# Implementation Plan

- [ ] 1. 共通ヘルパー関数を抽出する
- [ ] 1.1 マージコンフリクト解消セクション生成ロジックをプライベートヘルパーとして抽出する
  - 既存のマージコンフリクト解消セクション生成ロジックを独立したプライベート関数に切り出す
  - 挙動は変えずそのまま移植する（リファクタリングのみ、動作の変更なし）
  - _Requirements: 3.1_

- [ ] 1.2 (P) causes から指示文リストを生成するロジックをプライベートヘルパーとして抽出する
  - 既存の causes-to-instructions 変換ロジックを独立したプライベート関数に切り出す
  - 挙動は変えずそのまま移植する（リファクタリングのみ）
  - _Requirements: 3.2, 3.3_

- [ ] 1.3 (P) output-schema セクション生成ロジックをプライベートヘルパーとして抽出する
  - 既存の output-schema セクション生成ロジックを独立したプライベート関数に切り出す
  - 挙動は変えずそのまま移植する（リファクタリングのみ）
  - _Requirements: 3.2, 3.3_

- [ ] 2. DesignFixing 専用プロンプト生成を実装する
  - 設計ドキュメント修正に特化した文言（コード修正表現を除く）を含むプロンプトを生成する機能を追加する
  - コミット指示は `docs:` プレフィックスを用いる
  - 品質チェック指示として AGENTS.md / CLAUDE.md 参照の汎用指示を使用する
  - タスク 1 で抽出したヘルパーを呼び出してセクションを組み立てる
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [ ] 3. ImplementationFixing 専用プロンプト生成を実装する
  - 実装コード修正に特化した文言を含むプロンプトを生成する機能を追加する（現行と同等の内容）
  - コミット指示は `fix:` プレフィックスを用いる（現状維持）
  - タスク 1 で抽出したヘルパーを呼び出してセクションを組み立てる
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

- [ ] 4. セッション設定の分岐先を fixing 種別ごとのプロンプト生成に切り替える
  - `State::DesignFixing` では設計ドキュメント修正向けプロンプトを、`State::ImplementationFixing` では実装コード修正向けプロンプトを生成するよう分岐を変更する
  - 旧共通プロンプト生成関数を削除する
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
