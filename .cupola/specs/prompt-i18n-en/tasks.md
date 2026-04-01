# Implementation Plan

- [x] 1. JSON スキーマ定数の description を英語化する
- [x] 1.1 `PR_CREATION_SCHEMA` の description フィールドを英語化する
  - `pr_title`, `pr_body`, `feature_name` の各 `description` 値を英語文字列に置き換える
  - JSON プロパティ名は変更しない
  - `pr_creation_schema_is_valid_json` テストで JSON パースが通過することを確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 1.2 `FIXING_SCHEMA` の description フィールドを英語化する
  - `thread_id`, `response`, `resolved` の各 `description` 値を英語文字列に置き換える
  - JSON プロパティ名（`threads`, `thread_id`, `response`, `resolved`）は変更しない
  - `fixing_schema_is_valid_json` テストで JSON パースが通過することを確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 2. `GENERIC_QUALITY_CHECK_INSTRUCTION` 定数を英語化する
  - "Run quality checks described in AGENTS.md / CLAUDE.md before committing. If any check fails, fix the issues and re-run." 相当の英語文字列に置き換える
  - `AGENTS.md` / `CLAUDE.md` のファイル名参照を英語文字列内で維持する
  - commit 前チェック実行・失敗時の修正・再チェックの意味を英語で維持する
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` を参照する4件のテストは定数参照のため、定数変更のみで自動追従する（テスト側の変更不要）
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 3. `build_design_prompt` 関数のプロンプト本文を英語化する
  - プロンプトの見出し・説明・手順をすべて英語化する
  - 英語化後のプロンプトに設計エージェントとしての役割を示す英語記述が含まれることを確認する
  - `{quality_check}`, `{language}` フォーマットパラメータを維持する
  - `/kiro:spec-init`, `/kiro:spec-requirements`, `/kiro:spec-design`, `/kiro:spec-tasks` コマンド名を変更しない
  - `.cupola/inputs/issue.md`, `.cupola/specs/`, `.cupola/steering/` のパス参照を維持する
  - `Related: #{issue_number}` を維持し `Closes` を含めない
  - `design_running_returns_pr_creation_schema` テストの assertion を英語化後のエージェント識別文字列に更新する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

- [x] 4. `build_implementation_prompt` 関数のプロンプト本文を英語化する
  - プロンプトの見出し・説明・手順をすべて英語化する
  - 英語化後のプロンプトに実装エージェントとしての役割を示す英語記述が含まれることを確認する
  - `feature_name` が `None` の場合の `.cupola/specs/` 配下ディレクトリ特定手順を英語化する
  - `feature_name` が `Some(name)` の場合の `/kiro:spec-impl {name}` 行を維持する
  - `Closes #{issue_number}` を維持する
  - `{quality_check}`, `{language}`, `{feature_instruction}` フォーマットパラメータを維持する
  - `implementation_running_returns_pr_creation_schema` テストの assertion を英語化後のエージェント識別文字列に更新する
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

- [x] 5. `build_fixing_prompt` 関数のプロンプト本文を英語化する
- [x] 5.1 プロンプト本文（静的部分）を英語化する
  - プロンプトの見出し・説明・手順の静的部分をすべて英語化する
  - 英語化後のプロンプトにレビュー対応エージェントとしての役割を示す英語記述が含まれることを確認する
  - `{quality_check}`, `{language}`, `{instructions_text}` フォーマットパラメータを維持する
  - `design_fixing_returns_fixing_schema` テストの assertion を英語化後のエージェント識別文字列に更新する
  - _Requirements: 5.1, 5.6, 5.7, 8.1, 8.2, 8.7_

- [x] 5.2 動的文字列（各 `FixingProblemKind` 分岐）を英語化する
  - `ReviewComments` 分岐: `.cupola/inputs/review_threads.json` を参照する英語の指示文字列に変換する
  - `CiFailure` 分岐: `.cupola/inputs/ci_errors.txt` を参照する英語の指示文字列に変換する
  - `Conflict` 分岐: base branch のコンフリクト解消を指示する英語の文字列（"base branch" キーワードを含む）に変換する
  - フォールバック文字列（causes 空の場合）を英語化する
  - `origin/main` 等のブランチ名をハードコードしない（現状維持）
  - `fixing_prompt_conflict_only`, `fixing_prompt_all_causes` テストの assertion を英語化後の "base branch" 相当文字列に更新する
  - _Requirements: 5.2, 5.3, 5.4, 5.5, 5.8_

- [x] 6. `AGENTS.md` の品質チェック指示を英語化する
  - 品質チェック指示の本文を英語に書き換える
  - commit 前チェック実行・失敗時の修正・再チェックの意味を英語で維持する
  - 各リポジトリの設定やドキュメントへの参照に関する記述も英語化する
  - ファイル名 `AGENTS.md`, `CLAUDE.md` の参照を維持する
  - _Requirements: 7.1, 7.2, 7.3_

- [x] 7. 全テストの通過を確認する
  - `cargo test` を実行し全16件のテストが成功することを確認する
  - 失敗した場合は assertion の期待値と実際の英語化後プロンプト内容を照合して修正する
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_
