# Research & Design Decisions

## Summary
- **Feature**: `design-pr-closes-removal`
- **Discovery Scope**: Simple Addition（既存テンプレート文字列の修正）
- **Key Findings**:
  - `build_design_prompt` の PR body 出力指示に `Closes` / `Related` の明示的な指定が現在存在しない
  - `fallback_pr_body` は既に設計 PR で `Related` を使用しており、正しい動作をしている
  - `build_implementation_prompt` は `Closes #{issue_number} を含めること` と明示しており、こちらは変更不要

## Research Log

### 設計プロンプトにおける Issue 参照の現状分析
- **Context**: `build_design_prompt` が生成するプロンプトに `Closes` / `Related` の指定があるかを確認
- **Sources Consulted**: `src/application/prompt.rs`
- **Findings**:
  - `build_design_prompt`（L70-115）: PR body の出力指示に Issue 参照フォーマットの指定なし。Claude が自由に body を生成する
  - `build_implementation_prompt`（L117-157）: `Closes #{issue_number} を含めること` と明示的に指示（L151）
  - `fallback_pr_body`（L61-68）: 設計 PR → `Related: #N`、実装 PR → `Closes #N` と正しく分岐済み
- **Implications**:
  - `build_design_prompt` に `Related: #{issue_number}` の使用を明示的に指示し、`Closes` を使用しないよう制約を追加する必要がある
  - `fallback_pr_body` は変更不要（既に正しい）
  - `build_implementation_prompt` は変更不要（既に `Closes` を明示）

### PR body 内の GitHub キーワード動作
- **Context**: GitHub が PR body 内のどのキーワードで Issue を自動 close するか
- **Findings**:
  - GitHub は `Closes`, `Fixes`, `Resolves` などのキーワード + `#N` で Issue を自動 close する
  - `Related` はこれらのキーワードに含まれないため、自動 close を発生させない
- **Implications**: `Related: #N` を使用することで設計 PR マージ時の自動 close を確実に防止できる

## Design Decisions

### Decision: 設計プロンプトに `Related` 指示を追加
- **Context**: 設計 PR の body に `Closes` が含まれると Issue が自動 close される
- **Alternatives Considered**:
  1. プロンプトに `Related: #N` を含めるよう指示を追加する
  2. PR body のポストプロセスで `Closes` を `Related` に置換する
- **Selected Approach**: プロンプトに指示を追加する（選択肢 1）
- **Rationale**: 最もシンプルで、既存の `build_implementation_prompt` と同じパターン（L151）を踏襲できる
- **Trade-offs**: Claude が指示を無視する可能性はあるが、`fallback_pr_body` が最終的なセーフティネットとして機能する
- **Follow-up**: Claude の出力が指示に従っているかテストで確認する

## Risks & Mitigations
- リスク: Claude が指示を無視して `Closes` を含む body を生成する可能性 → `fallback_pr_body` が既に `Related` を使用しているためフォールバック時は安全。output パース成功時のみリスクあり
