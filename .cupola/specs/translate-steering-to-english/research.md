# Research & Design Decisions

## Summary
- **Feature**: `translate-steering-to-english`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - 翻訳対象は 3 ファイル（product.md, tech.md, structure.md）のみで、コード変更は不要
  - 既存の Markdown フォーマット・構造をそのまま維持する必要がある
  - 固有名詞（Cupola, Claude Code, cc-sdd 等）とコード識別子は翻訳対象外

## Research Log

### 翻訳対象ファイルの構造分析
- **Context**: 翻訳時に情報を落とさないために、各ファイルの構造を事前に把握する必要がある
- **Sources Consulted**: `.cupola/steering/product.md`, `tech.md`, `structure.md` の実ファイル
- **Findings**:
  - product.md: 4 セクション（概要、Core Capabilities、Target Use Cases、Value Proposition）
  - tech.md: 6 セクション + テーブル + コードブロック。技術用語が多く、誤訳リスクが最も高い
  - structure.md: 4 セクション。レイヤー名・型名・パス名などコード識別子が多い
- **Implications**: テーブルとコードブロックの構造を崩さない翻訳手順が必要

## Design Decisions

### Decision: ファイル単位の逐次翻訳
- **Context**: 3 ファイルの翻訳をどの粒度で管理するか
- **Alternatives Considered**:
  1. ファイル単位で逐次翻訳 — 各ファイルを独立して翻訳
  2. 全ファイル一括翻訳 — 一度に全ファイルを処理
- **Selected Approach**: ファイル単位の逐次翻訳
- **Rationale**: 各ファイルの性質が異なり（製品概要 vs 技術仕様 vs 構造説明）、個別に品質確認しやすい
- **Trade-offs**: 手順が増えるが、翻訳品質の検証が容易
- **Follow-up**: 翻訳後に原文との情報量比較を実施

## Risks & Mitigations
- 技術用語の誤訳 — 固有名詞・コード識別子は原文のまま維持するルールで対応
- Markdown フォーマットの崩れ — テーブル・コードブロック・見出しレベルの保持を検証項目に含める
- ニュアンスの喪失 — 直訳より意訳を優先し、技術的正確性を担保
