# Implementation Plan

- [ ] 1. issue/PR番号のコンテキストをプロンプトに追加する
- [ ] 1.1 `build_fixing_prompt` のパラメータ名からアンダースコアを除去し、ヘッダーに番号を埋め込む
  - `_issue_number` / `_pr_number` の先頭アンダースコアを除去して有効なパラメータとして使用する
  - プロンプト冒頭のヘッダー部分（"Actions Required" セクションの前）に `PR #<pr_number> (Issue #<issue_number>)` を挿入する
  - 両番号が実際の値として展開されることを確認する
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. output指示を causes の内容に応じて条件分岐させる
- [ ] 2.1 (P) `ReviewComments` 非含有ケースのoutputセクションを差し替える
  - `causes.contains(&FixingProblemKind::ReviewComments)` で真偽を判定する
  - `true` の場合: 現行のスレッドoutput指示（`thread_id` / `response` / `resolved`）を保持する
  - `false` の場合: スレッドoutput指示を省略し、`{"threads": []}` を返すよう明示したセクションに切り替える
  - output セクションのテンプレート文字列を変数化または条件式で分岐する
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 3. コミットステップの git add 指示を安全な形式に変更する
- [ ] 3.1 (P) `git add -A` を個別ファイル指定の案内に置き換える
  - プロンプト内の "Commit and push the fixes" ステップから `git add -A` を除去する
  - 代わりに `git diff --name-only` で変更ファイルを確認し、対象ファイルのみをステージングするよう明示する
  - 一時ファイルやデバッグログを誤ってステージングしないよう注意書きを追加する
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 4. 既存テストを更新し、3つの修正を網羅的に検証する
- [ ] 4.1 issue/PR番号の埋め込みを検証するアサーションを追加する
  - `fixing_prompt_review_comments_only` にPR番号・Issue番号がプロンプトに含まれることを検証するアサーションを追加する
  - `fixing_prompt_ci_failure_only` / `fixing_prompt_conflict_only` / `fixing_prompt_all_causes` にも同様のアサーションを追加する
  - _Requirements: 4.1_

- [ ] 4.2 (P) CI/コンフリクト専用ケースのoutput分岐を検証するアサーションを追加する
  - `fixing_prompt_ci_failure_only` で `{"threads": []}` が含まれ、`thread_id` などのスレッドoutput指示が含まれないことを検証する
  - `fixing_prompt_conflict_only` に同様のアサーションを追加する
  - `fixing_prompt_all_causes` でスレッドoutput指示が含まれることを検証する
  - _Requirements: 4.2, 4.3_

- [ ] 4.3 (P) `git add -A` の非存在を検証する新規テストを追加する
  - `fixing_prompt_no_git_add_all` テスト関数を追加し、`git add -A` がプロンプトに含まれないことをすべての causes パターンで検証する
  - _Requirements: 4.4_
