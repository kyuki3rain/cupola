# Requirements Document

## Introduction

Cupola の fixing フロー（`DesignFixing` / `ImplementationFixing`）では、共通の `build_fixing_prompt` 関数が使われているが、修正対象の性質が根本的に異なる。`DesignFixing` では `.cupola/specs/` 配下の設計ドキュメントが対象であり、コード修正を前提としたプロンプト文言・品質チェック・コミットメッセージが不適切である。本機能では、fixing 種別に応じたプロンプトを個別に生成できるよう `build_fixing_prompt` を分割し、各フローに最適化されたエージェント指示を提供する。

## Requirements

### Requirement 1: DesignFixing 専用プロンプト

**Objective:** As a Cupola 開発者, I want `DesignFixing` 状態向けの専用プロンプトを生成する機能, so that 設計ドキュメント修正エージェントが適切な指示を受け取れる

#### Acceptance Criteria

1. When `build_session_config` が `State::DesignFixing` で呼ばれた時, the prompt builder shall 設計ドキュメント修正に特化したプロンプトを生成する
2. The prompt builder shall 「設計ドキュメントを修正する」旨の指示文を含むプロンプトを生成する（「コードを修正する」旨の表現を含めない）
3. The prompt builder shall `docs:` プレフィックスのコミットメッセージ指示を含むプロンプトを生成する
4. The prompt builder shall ドキュメント整合性向けの品質チェック指示を含むプロンプトを生成する（clippy / cargo test 等のコード向けチェックを含めない）
5. When `FixingProblemKind::ReviewComments` が causes に含まれる時, the prompt builder shall レビューコメントを `.cupola/specs/` 配下のドキュメントに反映する指示を含む
6. When `FixingProblemKind::CiFailure` が causes に含まれる時, the prompt builder shall CI エラーを設計ドキュメントの観点で確認・修正する指示を含む
7. If `has_merge_conflict` が true の時, the prompt builder shall マージコンフリクト解消セクションをプロンプト先頭に挿入する

### Requirement 2: ImplementationFixing 専用プロンプト

**Objective:** As a Cupola 開発者, I want `ImplementationFixing` 状態向けの専用プロンプトを生成する機能, so that 実装コード修正エージェントが現状相当の適切な指示を受け取れる

#### Acceptance Criteria

1. When `build_session_config` が `State::ImplementationFixing` で呼ばれた時, the prompt builder shall 実装コード修正に特化したプロンプトを生成する
2. The prompt builder shall 「コードを修正する」旨の指示文を含むプロンプトを生成する
3. The prompt builder shall `fix:` プレフィックスのコミットメッセージ指示を含むプロンプトを生成する
4. The prompt builder shall コード向け品質チェック指示（AGENTS.md / CLAUDE.md の品質チェック）を含むプロンプトを生成する
5. When `FixingProblemKind::ReviewComments` が causes に含まれる時, the prompt builder shall `review_threads.json` を参照してレビューコメントを実装コードに反映する指示を含む
6. When `FixingProblemKind::CiFailure` が causes に含まれる時, the prompt builder shall `ci_errors.txt` を参照して CI 失敗を修正する指示を含む
7. If `has_merge_conflict` が true の時, the prompt builder shall マージコンフリクト解消セクションをプロンプト先頭に挿入する

### Requirement 3: 共通ロジックの保持

**Objective:** As a Cupola 開発者, I want 分割後も共通動作（マージコンフリクト処理・レビュースレッド出力・causes 解析）を維持する, so that 既存テストが継続して通過し挙動の退行がない

#### Acceptance Criteria

1. The prompt builder shall マージコンフリクト解消セクションの生成ロジックを `DesignFixing` / `ImplementationFixing` の両方に適用する
2. The prompt builder shall `FixingProblemKind::ReviewComments` が含まれる場合の output-schema セクション（thread_id / response / resolved）を両プロンプトで正しく生成する
3. The prompt builder shall `FixingProblemKind::ReviewComments` が含まれない場合に `{"threads": []}` を返す output-schema を両プロンプトで生成する
4. The prompt builder shall `git add -A` を使わず変更ファイルを個別に stage する指示を両プロンプトで含める
5. While `State::DesignFixing` または `State::ImplementationFixing` のいずれかで `pr_number` が None の時, the prompt builder shall `anyhow::Error` を返す

### Requirement 4: テストカバレッジ

**Objective:** As a Cupola 開発者, I want 各プロンプト種別のユニットテストを整備する, so that 回帰を防止し仕様の文書化代わりになる

#### Acceptance Criteria

1. The test suite shall `DesignFixing` が設計ドキュメント向けの指示を含み「code」向け表現を含まないことを検証する
2. The test suite shall `ImplementationFixing` が実装コード向けの指示を含むことを検証する
3. The test suite shall 各 `FixingProblemKind` の組み合わせで両プロンプトが正しく生成されることを検証する
4. The test suite shall コミットメッセージプレフィックス（`docs:` / `fix:`）が正しく指示されることを検証する
