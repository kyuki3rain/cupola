# Design Document

## Overview

**Purpose**: 設計 PR の body から `Closes #N` を除外し、`Related #N` のみを使用することで、設計 PR マージ時の Issue 自動 close を防止する。

**Users**: Cupola を使用する開発者。設計 PR と実装 PR の Issue ライフサイクルを適切に管理するために利用する。

**Impact**: `build_design_prompt` が生成するプロンプトテンプレートに `Related` の使用指示を追加し、`Closes` の使用を禁止する制約を追加する。

### Goals
- 設計 PR の body で `Related #N` を使用し、`Closes #N` を含めない
- 実装 PR の body は現状通り `Closes #N` を維持する
- 既存の `fallback_pr_body` の動作は変更しない

### Non-Goals
- PR body のポストプロセスによる `Closes` → `Related` 置換は行わない
- `fallback_pr_body` の変更は行わない（既に正しい動作をしている）
- `build_fixing_prompt` への変更は行わない

## Architecture

### Existing Architecture Analysis

変更対象は `src/application/prompt.rs` の `build_design_prompt` 関数のみ。

- **現状**: `build_design_prompt`（L70-115）は PR body の出力指示に Issue 参照フォーマットの指定がない
- **対比**: `build_implementation_prompt`（L151）は `Closes #{issue_number} を含めること` と明示的に指示している
- **安全装置**: `fallback_pr_body`（L61-68）は設計 PR で `Related: #N` を既に使用しており、Claude の出力パース失敗時のフォールバックとして機能する

### Architecture Pattern & Boundary Map

既存アーキテクチャパターンを変更しない。application 層内のプロンプトテンプレート修正のみ。

## Requirements Traceability

| Requirement | Summary | Components | Interfaces | Flows |
|-------------|---------|------------|------------|-------|
| 1.1 | 設計プロンプトで `Related #N` を使用 | `build_design_prompt` | なし | なし |
| 1.2 | 設計 PR body に `Closes` を含めない | `build_design_prompt` | なし | なし |
| 1.3 | 設計 PR マージ時に Issue を自動 close しない | `build_design_prompt` | なし | なし |
| 2.1 | 実装プロンプトで `Closes #N` を維持 | `build_implementation_prompt` | なし | なし |
| 2.2 | 実装 PR body の `Closes #N` を変更しない | `build_implementation_prompt` | なし | なし |

## Components and Interfaces

| Component | Domain/Layer | Intent | Req Coverage | Key Dependencies | Contracts |
|-----------|-------------|--------|--------------|------------------|-----------|
| `build_design_prompt` | application | 設計エージェント向けプロンプト生成 | 1.1, 1.2, 1.3 | なし | なし |
| `build_implementation_prompt` | application | 実装エージェント向けプロンプト生成（変更なし） | 2.1, 2.2 | なし | なし |

### Application Layer

#### `build_design_prompt`

| Field | Detail |
|-------|--------|
| Intent | 設計エージェント向けプロンプトの PR body 出力指示に `Related` の使用と `Closes` の禁止を追加する |
| Requirements | 1.1, 1.2, 1.3 |

**Responsibilities & Constraints**
- PR body の `output-schema への出力` セクションに `Related #{issue_number}` を含める指示を追加する
- `制約事項` セクションに `Closes` を使用しない旨の制約を追加する

**Implementation Notes**
- 変更箇所: `src/application/prompt.rs` の `build_design_prompt` 関数内のフォーマット文字列
- `build_implementation_prompt` の L151 と同様のパターンで指示を追加する
- `fallback_pr_body` は変更不要（既に `Related: #N` を使用）

## Testing Strategy

### Unit Tests
- `build_design_prompt` の出力に `Related` 指示が含まれることを確認する
- `build_design_prompt` の出力に `Closes` が含まれないことを確認する
- `build_implementation_prompt` の出力に `Closes` 指示が引き続き含まれることを確認する（回帰テスト）
- `fallback_pr_body` の既存テストが引き続きパスすることを確認する（回帰テスト）
