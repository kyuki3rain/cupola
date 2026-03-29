# Implementation Plan

- [ ] 1. 設計プロンプトに Issue 参照フォーマットの指示を追加する
- [ ] 1.1 設計プロンプトの PR body 出力指示に `Related` の使用と `Closes` の禁止を追加する
  - `output-schema への出力` セクションの `pr_body` 指示に `Related #{issue_number}` を含める旨を追加する
  - `制約事項` セクションに `Closes` を PR body に含めない旨の制約を追加する
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. テストを追加・更新して要件を検証する
- [ ] 2.1 設計プロンプトに `Related` 指示が含まれ `Closes` が含まれないことを検証するテストを追加する
  - 設計プロンプトの出力に `Related` の使用指示が含まれることを確認する
  - 設計プロンプトの出力に `Closes` が含まれないことを確認する
  - _Requirements: 1.1, 1.2_

- [ ] 2.2 (P) 実装プロンプトが `Closes` 指示を維持していることを検証する回帰テストを追加する
  - 実装プロンプトの出力に `Closes` が引き続き含まれることを確認する
  - 既存の `fallback_pr_body` テストが変更なしでパスすることを確認する
  - _Requirements: 2.1, 2.2_
