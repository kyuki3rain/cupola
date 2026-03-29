# Research & Design Decisions

## Summary
- **Feature**: `github-issue-template`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - GitHub Issue Forms は YAML ベースのフォーム定義で、`textarea`、`input`、`markdown` などのフィールドタイプをサポートする
  - `validations.required: true` を設定することで GitHub 側でバリデーションが行われ、未入力の Issue 作成を防止できる
  - `title` プロパティの `default` にプレフィックスを設定可能だが、ラベルの自動付与は `labels` プロパティで制御する

## Research Log

### GitHub Issue Forms YAML 仕様
- **Context**: テンプレートの YAML 構造と利用可能なフィールドタイプの確認
- **Sources Consulted**: GitHub Docs — Issue Forms syntax
- **Findings**:
  - トップレベルプロパティ: `name`, `description`, `title`, `labels`, `assignees`, `body`
  - `body` は配列で、各要素に `type`（`markdown`, `textarea`, `input`, `dropdown`, `checkboxes`）を指定
  - `textarea` は複数行テキスト入力に適し、`placeholder` と `description` で入力ガイダンスを提供可能
  - `validations.required: true` で必須フィールドを定義
- **Implications**: 全フィールドを `textarea` タイプで定義し、必須/任意を `validations.required` で制御する

## Design Decisions

### Decision: フィールドタイプの選択
- **Context**: 各フィールドに適切な入力タイプを選択する必要がある
- **Alternatives Considered**:
  1. `input` — 単一行テキスト入力
  2. `textarea` — 複数行テキスト入力
- **Selected Approach**: 全フィールドに `textarea` を使用
- **Rationale**: 概要・背景・要求事項・スコープ等はいずれも複数行の記述が想定されるため、`textarea` が適切
- **Trade-offs**: 短い入力（タイトルプレフィックス等）には冗長だが、一貫性を優先
- **Follow-up**: なし

### Decision: ラベルの自動付与を行わない
- **Context**: Issue の `agent:ready` ラベルは人間が明示的に付与する運用
- **Alternatives Considered**:
  1. `labels` プロパティで `agent:ready` を自動付与
  2. ラベルを自動付与しない
- **Selected Approach**: `labels` プロパティを空にするか省略する
- **Rationale**: 要件定義で明示的に「自動付与しない」と指定されている。人間がレビュー後にラベルを付与するワークフローを維持する
- **Trade-offs**: ユーザーが手動でラベルを付ける手間が発生するが、意図しない自動処理の開始を防止できる
- **Follow-up**: なし

## Risks & Mitigations
- テンプレートの YAML 構文エラー — GitHub の Issue 作成画面で事前にテンプレート表示を確認する
- フィールドの過不足 — cc-sdd の requirements フェーズで実際に使用して入力情報の十分性を検証する

## References
- GitHub Docs: Issue Forms syntax — テンプレート YAML の公式仕様
