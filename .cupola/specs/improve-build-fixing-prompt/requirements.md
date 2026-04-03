# Requirements Document

## Project Description (Input)
build_fixing_prompt の改善: issue/PR番号のコンテキスト追加、output指示の条件分岐、git add -A の安全化

## Introduction

`src/application/prompt.rs` の `build_fixing_prompt` 関数には、Claude Code エージェントに渡すプロンプトの品質を低下させる3つの問題がある。本仕様は、それら3点を修正することで、修正プロンプトの正確性・安全性・一貫性を向上させる。

## Requirements

### Requirement 1: Issue/PR番号のコンテキスト追加

**Objective:** 自動レビューエージェントとして、修正対象のPR番号とIssue番号をプロンプト本文に含めたい。そうすることで、Claude Code が「どのIssueの、どのPRを修正しているか」を把握し、正確なコミットメッセージ生成と一貫した修正を行えるようにする。

#### Acceptance Criteria
1. When `build_fixing_prompt` が呼び出されるとき、the Prompt Builder shall `_issue_number` および `_pr_number` パラメータの先頭アンダースコアを除去し、実際の値として参照する
2. The Prompt Builder shall プロンプト本文のヘッダー部分に `PR #<pr_number> (Issue #<issue_number})` の形式でコンテキスト情報を含める
3. When `build_session_config` が `DesignFixing` または `ImplementationFixing` 状態で呼び出されるとき、the Prompt Builder shall `issue_number` と `pr_number` の両方をプロンプトテキストに埋め込む

---

### Requirement 2: output指示のcauses依存条件分岐

**Objective:** 自動レビューエージェントとして、修正内容に応じたoutput指示のみをプロンプトに含めたい。そうすることで、CIエラー・コンフリクト専用の修正時にレビュースレッドのoutput要求が混入せず、Claude Code が不要な出力を生成しないようにする。

#### Acceptance Criteria
1. When `causes` に `ReviewComments` が含まれるとき、the Prompt Builder shall スレッドごとの `thread_id`・`response`・`resolved` フィールドのoutput指示をプロンプトに含める
2. When `causes` に `ReviewComments` が含まれないとき（CI失敗のみ・コンフリクトのみ等）、the Prompt Builder shall スレッドoutput指示を省略し、代わりに `{"threads": []}` を返すよう明示する
3. The Prompt Builder shall `ReviewComments` を含まないケースのoutput セクションに、スレッド出力は不要である旨を明記する

---

### Requirement 3: git add のスコープ限定

**Objective:** 自動レビューエージェントとして、修正プロンプト内の `git add -A` を安全なステージング指示に変更したい。そうすることで、一時ファイルやデバッグログが誤ってコミットされるリスクを排除し、design/implementation プロンプトとの一貫性を確保する。

#### Acceptance Criteria
1. The Prompt Builder shall 修正プロンプトの "Commit and push the fixes" ステップにおいて `git add -A` の代わりに、変更したファイルのみをステージングするよう明示した指示を含める
2. The Prompt Builder shall `git add` の使い方について、意図しないファイルを含めないよう具体的なガイダンス（例: `git add <changed_files>` または `git diff --name-only` で確認してからステージング）を提供する
3. If プロンプトが `git add -A` を含む場合、the Prompt Builder shall ビルドまたはテストで失敗する（後退防止）

---

### Requirement 4: 既存テストの更新

**Objective:** 開発者として、上記3つの変更を検証する既存テストを更新し新たなアサーションを追加したい。そうすることで、リグレッションを防ぎ変更の正確性を継続的に保証できるようにする。

#### Acceptance Criteria
1. When `fixing_prompt_review_comments_only` テストが実行されるとき、the Test Suite shall プロンプトにPR番号・Issue番号が含まれることを検証する
2. When `fixing_prompt_ci_failure_only` または `fixing_prompt_conflict_only` テストが実行されるとき、the Test Suite shall `{"threads": []}` の記述がプロンプトに含まれ、スレッドoutput指示が含まれないことを検証する
3. When `fixing_prompt_all_causes` テストが実行されるとき、the Test Suite shall スレッドoutput指示と `{"threads": []}` の両方の挙動を正しく検証する
4. The Test Suite shall `git add -A` がプロンプトに含まれないことを検証するテストケースを持つ
