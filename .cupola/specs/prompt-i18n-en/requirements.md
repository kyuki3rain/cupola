# Requirements Document

## Project Description (Input)
内部プロンプトの英語化（トークン削減）: src/application/prompt.rs の定数・関数・テストおよび AGENTS.md における日本語プロンプトを英語に変換し、トークン消費を削減する。出力言語は {language} プレースホルダで制御を維持する。

## はじめに

Cupola は AI エージェント（Claude Code）に対して内部プロンプトを送信し、設計・実装・レビュー対応の各タスクを自動化する。現在、`src/application/prompt.rs` の定数・関数本文、および `AGENTS.md` の品質チェック指示が日本語で記述されており、AI への送信トークン数が増大している。本スペックでは、これらの内部プロンプトをすべて英語化してトークン消費を削減する。出力言語（PR body、コメント返信、cc-sdd ドキュメント等）は `{language}` プレースホルダにより引き続き制御する。

## Requirements

### Requirement 1: JSON スキーマ定数の英語化

**Objective:** As a システム開発者, I want `PR_CREATION_SCHEMA` および `FIXING_SCHEMA` の JSON schema description フィールドを英語化する, so that AI への送信トークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall have `PR_CREATION_SCHEMA` の `description` フィールドがすべて英語文字列となっている。
2. The Cupola system shall have `FIXING_SCHEMA` の各 `description` フィールドがすべて英語文字列となっている。
3. The Cupola system shall `PR_CREATION_SCHEMA` に含まれるプロパティ名（`pr_title`, `pr_body`, `feature_name`）を変更しない。
4. The Cupola system shall `FIXING_SCHEMA` に含まれるプロパティ名（`threads`, `thread_id`, `response`, `resolved`）を変更しない。
5. When `PR_CREATION_SCHEMA` または `FIXING_SCHEMA` を JSON としてパースする, the Cupola system shall 有効な JSON オブジェクトとして解析できる。

---

### Requirement 2: GENERIC_QUALITY_CHECK_INSTRUCTION 定数の英語化

**Objective:** As a システム開発者, I want `GENERIC_QUALITY_CHECK_INSTRUCTION` 定数を英語化する, so that 品質チェック指示のトークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall `GENERIC_QUALITY_CHECK_INSTRUCTION` の文字列が英語で記述されている。
2. The Cupola system shall `GENERIC_QUALITY_CHECK_INSTRUCTION` が `AGENTS.md` / `CLAUDE.md` への参照を維持している。
3. The Cupola system shall `GENERIC_QUALITY_CHECK_INSTRUCTION` が commit 前の品質チェック実行とチェック失敗時の修正・再チェックの指示を含む。

---

### Requirement 3: build_design_prompt 関数の英語化

**Objective:** As a システム開発者, I want `build_design_prompt` 関数のプロンプト本文を英語化する, so that 設計エージェントへの送信トークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall `build_design_prompt` が返すプロンプトに、設計エージェントとしての役割を示す英語の記述が含まれている。
2. The Cupola system shall `build_design_prompt` が返すプロンプトに `/kiro:spec-init`, `/kiro:spec-requirements`, `/kiro:spec-design`, `/kiro:spec-tasks` コマンド名がそのまま含まれている。
3. The Cupola system shall `build_design_prompt` が返すプロンプトに `.cupola/inputs/issue.md`, `.cupola/specs/`, `.cupola/steering/` のパス参照が維持されている。
4. The Cupola system shall `build_design_prompt` が返すプロンプトに `Related: #{issue_number}` が含まれており、`Closes` が含まれていない。
5. The Cupola system shall `build_design_prompt` が返すプロンプトに `{quality_check}` フォーマットパラメータ経由で `GENERIC_QUALITY_CHECK_INSTRUCTION` の内容が展開されている。
6. The Cupola system shall `build_design_prompt` が返すプロンプトに `{language}` プレースホルダが出力言語指示として維持されている。

---

### Requirement 4: build_implementation_prompt 関数の英語化

**Objective:** As a システム開発者, I want `build_implementation_prompt` 関数のプロンプト本文を英語化する, so that 実装エージェントへの送信トークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall `build_implementation_prompt` が返すプロンプトに、実装エージェントとしての役割を示す英語の記述が含まれている。
2. The Cupola system shall `build_implementation_prompt` が返すプロンプトに `/kiro:spec-impl` コマンド名がそのまま含まれている。
3. The Cupola system shall `build_implementation_prompt` が返すプロンプトに `Closes #{issue_number}` が含まれている。
4. The Cupola system shall `build_implementation_prompt` が返すプロンプトに `{quality_check}` フォーマットパラメータ経由で `GENERIC_QUALITY_CHECK_INSTRUCTION` の内容が展開されている。
5. The Cupola system shall `build_implementation_prompt` が返すプロンプトに `{language}` プレースホルダが出力言語指示として維持されている。
6. When `feature_name` が `None` の場合, the Cupola system shall `build_implementation_prompt` が返すプロンプトに `.cupola/specs/` 配下のディレクトリを特定する手順が英語で記述されている。
7. When `feature_name` が `Some(name)` の場合, the Cupola system shall `build_implementation_prompt` が返すプロンプトに `/kiro:spec-impl {name}` が含まれている。

---

### Requirement 5: build_fixing_prompt 関数の英語化

**Objective:** As a システム開発者, I want `build_fixing_prompt` 関数のプロンプト本文（静的部分・動的部分のすべて）を英語化する, so that レビュー対応エージェントへの送信トークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall `build_fixing_prompt` が返すプロンプトに、レビュー対応エージェントとしての役割を示す英語の記述が含まれている。
2. When `causes` に `FixingProblemKind::ReviewComments` が含まれる場合, the Cupola system shall `.cupola/inputs/review_threads.json` を参照する英語の指示がプロンプトに含まれる。
3. When `causes` に `FixingProblemKind::CiFailure` が含まれる場合, the Cupola system shall `.cupola/inputs/ci_errors.txt` を参照する英語の指示がプロンプトに含まれる。
4. When `causes` に `FixingProblemKind::Conflict` が含まれる場合, the Cupola system shall base ブランチのコンフリクト解消を指示する英語の文字列（"base branch" 相当）がプロンプトに含まれる。
5. When `causes` が空の場合, the Cupola system shall フォールバック文字列が英語でプロンプトに含まれる。
6. The Cupola system shall `build_fixing_prompt` が返すプロンプトに `{quality_check}` フォーマットパラメータ経由で `GENERIC_QUALITY_CHECK_INSTRUCTION` の内容が展開されている。
7. The Cupola system shall `build_fixing_prompt` が返すプロンプトに `{language}` プレースホルダが出力言語指示として維持されている。
8. The Cupola system shall `build_fixing_prompt` が返すプロンプトに `origin/main` などブランチ名のハードコードが含まれていない。

---

### Requirement 6: テストの更新

**Objective:** As a システム開発者, I want 既存の16件のテストを英語化されたプロンプトに合わせて更新する, so that `cargo test` が成功する。

#### Acceptance Criteria

1. The Cupola system shall `design_running_returns_pr_creation_schema` テストが、英語化後の設計エージェント識別文字列を assertion として使用する。
2. The Cupola system shall `implementation_running_returns_pr_creation_schema` テストが、英語化後の実装エージェント識別文字列を assertion として使用する。
3. The Cupola system shall `design_fixing_returns_fixing_schema` テストが、英語化後のレビュー対応エージェント識別文字列を assertion として使用する。
4. The Cupola system shall `fixing_prompt_conflict_only` テストが、英語化後の base ブランチ識別文字列（"base branch" 相当）を assertion として使用する。
5. The Cupola system shall `fixing_prompt_all_causes` テストが、英語化後の base ブランチ識別文字列を assertion として使用する。
6. The Cupola system shall `GENERIC_QUALITY_CHECK_INSTRUCTION` を参照する4件のテスト（`design_prompt_generic_quality_check`, `implementation_prompt_generic_quality_check`, `implementation_prompt_without_feature_name_generic_quality_check`, `fixing_prompt_generic_quality_check`）が、定数変更後も引き続き `session.prompt.contains(GENERIC_QUALITY_CHECK_INSTRUCTION)` で通過する。
7. When `cargo test` を実行する, the Cupola system shall 全16件のテストが成功する。

---

### Requirement 7: AGENTS.md の英語化

**Objective:** As a システム開発者, I want `AGENTS.md` の品質チェック指示を英語化する, so that AI に送信される品質チェック指示のトークンを削減できる。

#### Acceptance Criteria

1. The Cupola system shall `AGENTS.md` の品質チェック指示本文が英語で記述されている。
2. The Cupola system shall `AGENTS.md` が commit 前の品質チェック実行と失敗時の修正・再チェックの指示を英語で維持している。
3. The Cupola system shall `AGENTS.md` のファイル名参照（`AGENTS.md`, `CLAUDE.md`）が維持されている。

---

### Requirement 8: 不変項目の保護

**Objective:** As a システム開発者, I want 英語化に際して変更禁止の要素が一切変更されないことを保証する, so that 既存の動作（出力言語制御・コマンド・パス・プロパティ名）が破壊されない。

#### Acceptance Criteria

1. The Cupola system shall `{language}` プレースホルダがすべてのプロンプト関数において維持されている。
2. The Cupola system shall `{quality_check}`, `{feature_instruction}`, `{instructions_text}` などのフォーマットパラメータが変更されていない。
3. The Cupola system shall `/kiro:spec-init`, `/kiro:spec-requirements`, `/kiro:spec-design`, `/kiro:spec-tasks`, `/kiro:spec-impl` コマンド名が変更されていない。
4. The Cupola system shall `.cupola/inputs/`, `.cupola/specs/`, `.cupola/steering/` のパスが変更されていない。
5. The Cupola system shall `Related:`, `Closes` の GitHub キーワードが変更されていない。
6. The Cupola system shall JSON schema のプロパティ名（`pr_title`, `pr_body`, `feature_name`, `threads`, `thread_id`, `response`, `resolved`）が変更されていない。
7. The Cupola system shall `AGENTS.md`, `CLAUDE.md` のファイル名参照が変更されていない。
