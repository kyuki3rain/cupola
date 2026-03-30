# Requirements Document

## Introduction
設計 PR の body フォーマットから `Closes #N` を除外し、`Related: #N` のみにする変更。設計 PR は中間成果物であり Issue を close すべきではないため、`build_design_prompt` の PR body テンプレートを修正する。実装 PR の `Closes #N` は現状通り維持する。

## Requirements

### Requirement 1: 設計 PR body の Issue 参照フォーマット変更
**Objective:** As a 開発者, I want 設計 PR の body で `Closes #N` の代わりに `Related: #N` を使用したい, so that 設計 PR マージ時に Issue が自動 close されることを防止できる

#### Acceptance Criteria
1. When 設計プロンプト（`build_design_prompt`）が PR body を生成する場合, the Cupola shall PR body 内の Issue 参照に `Related: #N` フォーマットを使用する
2. The Cupola shall 設計 PR の body に `Closes` キーワードを含めない
3. When 設計 PR がマージされた場合, the Cupola shall 関連する Issue を自動 close しない（`Related` キーワードにより GitHub が自動 close を実行しないことを保証する）

### Requirement 2: 実装 PR body の Issue 参照フォーマット維持
**Objective:** As a 開発者, I want 実装 PR の body では引き続き `Closes #N` を使用したい, so that 実装 PR マージ時に Issue が自動 close される現行動作を維持できる

#### Acceptance Criteria
1. When 実装プロンプト（`build_implementation_prompt`）が PR body を生成する場合, the Cupola shall PR body 内の Issue 参照に `Closes #N` フォーマットを引き続き使用する
2. The Cupola shall 実装 PR の body における `Closes #N` の動作を変更しない
