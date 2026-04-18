# Implementation Plan

- [x] 1. `/cupola:fix` スキルの design/impl 対応
- [x] 1.1 design/impl で適切なコミットプレフィックスと動的メッセージを生成できるようにする
  - `design` 引数時は `docs:` プレフィックス、`impl` 引数時は `fix:` プレフィックスを付与する
  - 固定文字列のコミットメッセージ本文を廃止し、変更内容に基づいて AI が動的にメッセージを生成するよう指示する
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 2. `prompt.rs` の fixing プロンプト簡略化
- [x] 2.1 `build_conflict_section` と `build_instructions_text` ヘルパー関数を削除する
  - `build_conflict_section` を削除する（スキルが自律的にコンフリクト検出・対応するため）
  - `build_instructions_text` を削除する（スキルが入力ファイルの有無で自律的に判断するため）
  - _Requirements: 1.3, 1.6_

- [x] 2.2 `build_design_fixing_prompt` をスキル委譲形式に書き換える
  - 関数シグネチャから `has_merge_conflict: bool` および `default_branch: &str` を除去する
  - `build_conflict_section` と `build_instructions_text` の呼び出しを削除する
  - `## Actions Required` と `## Steps` セクションを削除する
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` への参照を削除する
  - `Run /cupola:fix design` の呼び出し指示を追加する
  - PR番号・Issue番号・言語コンテキスト情報を維持する
  - `build_output_section` の呼び出しを維持する
  - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.6, 1.7_

- [x] 2.3 `build_implementation_fixing_prompt` をスキル委譲形式に書き換える
  - 2.2 と同じ手順で `impl` 向けに実装する（`Run /cupola:fix impl`）
  - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [x] 2.4 `build_session_config` のシグネチャから `has_merge_conflict` を除去する
  - 関数シグネチャから `has_merge_conflict: bool` を削除する
  - `build_design_fixing_prompt` / `build_implementation_fixing_prompt` への転送引数から除去する
  - _Requirements: 1.7_

- [x] 3. `execute.rs` の呼び出し側を更新する
- [x] 3.1 `build_session_config` 呼び出しから `has_merge_conflict` 引数を除去する
  - `execute.rs` の `build_session_config(...)` 呼び出しから `has_merge_conflict` 引数を削除する
  - `has_merge_conflict` 変数の宣言・代入（`let mut has_merge_conflict = false;` および `has_merge_conflict = true;`）を削除する
  - マージ処理（`worktree.merge`）と警告ログは維持する
  - _Requirements: 1.7_

- [x] 4. テストの更新
- [x] 4.1 削除済みインライン手順を検証していたテストを削除する
  - `fixing_prompt_has_merge_conflict_true_inserts_section_at_top` を削除する
  - `fixing_prompt_has_merge_conflict_false_is_unchanged` を削除する
  - `design_fixing_prompt_contains_docs_prefix` を削除する（コミットメッセージは prompt に含まれなくなる）
  - `implementation_fixing_prompt_contains_fix_prefix` を削除する（同上）
  - `design_fixing_prompt_does_not_contain_code_wording` を削除する
  - `implementation_fixing_prompt_contains_code_wording` を削除する
  - `fixing_prompt_review_comments_only` / `fixing_prompt_ci_failure_only` から `review_threads.json` / `ci_errors.txt` アサーションを削除する（スキルが対応するため prompt には含まれない）
  - _Requirements: 3.2, 3.3_

- [x] 4.2 スキル委譲を検証する新規テストを追加する
  - `design_fixing_prompt_delegates_to_fix_skill`: `Run /cupola:fix design` がプロンプトに含まれることを検証する
  - `implementation_fixing_prompt_delegates_to_fix_skill`: `Run /cupola:fix impl` がプロンプトに含まれることを検証する
  - `design_fixing_prompt_has_no_inline_steps`: `git commit -m` / `git add` 等のインライン手順がプロンプトに含まれないことを検証する
  - `implementation_fixing_prompt_has_no_inline_steps`: 同上
  - _Requirements: 3.1, 3.2_

- [x] 4.3 output-schema セクションの検証テストを維持・更新する
  - `thread_id`・`response`・`resolved` の出力仕様が ReviewComments 時に含まれることを確認する
  - `{"threads": []}` が ReviewComments なし時に含まれることを確認する
  - `build_session_config` のシグネチャ変更に合わせてテスト呼び出しから `has_merge_conflict` 引数を除去する
  - _Requirements: 3.4_
