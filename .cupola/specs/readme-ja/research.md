# Research & Design Decisions

## Summary
- **Feature**: `readme-ja`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - README.md は全 230 行、8 セクション構成。技術用語・コマンド・コード例が多く、翻訳時にこれらを原文維持する方針が重要
  - GitHub の Markdown レンダリングでは相対パスリンク（`./README.ja.md`）が正しく動作する
  - 日本語版アンカーリンクは日本語見出しに対応させる必要がある（GitHub は日本語見出しからアンカーを自動生成する）

## Research Log

### GitHub における多言語 README の慣行
- **Context**: 言語切り替えリンクの配置パターンを確認
- **Findings**:
  - タイトル直下に `[English](./README.md) | [日本語](./README.ja.md)` の形式で配置するのが一般的
  - 相対パスで同一ディレクトリ内のファイルを参照する
- **Implications**: リンク形式はシンプルなインラインリンクで十分

### 日本語 Markdown アンカーリンクの挙動
- **Context**: 目次のアンカーリンクが日本語見出しで正しく動作するか確認
- **Findings**:
  - GitHub は日本語見出しからアンカーを自動生成する（例: `## プロジェクト概要` → `#プロジェクト概要`）
  - 英語見出しをそのまま使う場合はアンカーも英語のまま
- **Implications**: 見出しを日本語化する場合、目次のアンカーリンクも日本語に合わせて更新する

## Design Decisions

### Decision: 言語切り替えリンクの配置位置
- **Context**: README.md と README.ja.md の相互リンクをどこに配置するか
- **Alternatives Considered**:
  1. タイトル直下にインラインで配置
  2. 目次の前に独立セクションとして配置
- **Selected Approach**: タイトル直下にインラインで配置
- **Rationale**: 最も視認性が高く、OSS プロジェクトで広く採用されているパターン
- **Trade-offs**: シンプルだが、3 言語以上に拡張する場合はセクション化が必要になる可能性
- **Follow-up**: なし

### Decision: 技術用語の翻訳方針
- **Context**: コマンド名、設定キー名、ライブラリ名等の技術用語をどう扱うか
- **Alternatives Considered**:
  1. 全て原文維持
  2. 一般的な用語のみ翻訳（例: "Build" → "ビルド"）
- **Selected Approach**: コードブロック・コマンド・設定キー・ライブラリ名は原文維持。説明文中の一般的な技術用語（Build, Test 等）は文脈に応じて日本語化
- **Rationale**: 技術ドキュメントとしての正確性を維持しつつ、日本語話者にとって自然な読み心地を実現
- **Trade-offs**: 一部用語の翻訳判断が主観的になる
- **Follow-up**: 実装時にセクションごとに翻訳品質を確認

## Risks & Mitigations
- README.md の更新時に README.ja.md が追従しない可能性 — Issue テンプレートや CONTRIBUTING ガイドで同時更新ルールを明記（本スコープ外）
- 日本語アンカーリンクの互換性 — GitHub 標準のアンカー生成に準拠することで回避
