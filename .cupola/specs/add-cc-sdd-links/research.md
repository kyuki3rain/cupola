# Research & Design Decisions

---
**Purpose**: 調査結果・アーキテクチャ検討・設計根拠を記録する。

---

## Summary

- **Feature**: `add-cc-sdd-links`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - 対象5ファイルはすべて既存ファイルへの最小限の文字列置換で対応可能（新規コンポーネント不要）
  - YAML ファイルはマークダウンリンク記法を解釈しないため、プレーンテキスト形式の URL 併記が適切
  - 各ファイルで cc-sdd の初出箇所は1箇所のみに限定されており、変更範囲が明確

## Research Log

### 対象ファイル現状確認

- **Context**: cc-sdd が言及されている箇所と現在の記法を把握するために調査
- **Sources Consulted**: 各ファイルを直接参照
- **Findings**:
  - `README.md` L44: `**cc-sdd (spec-driven development)**` — インライン強調テキスト
  - `README.ja.md` L44: `**cc-sdd（仕様駆動開発）**` — インライン強調テキスト
  - `CHANGELOG.md` L14: `using cc-sdd` — Changelog エントリの文中
  - `.github/ISSUE_TEMPLATE/cupola-task.yml` L10: `cc-sdd の requirements フェーズ` — YAML の `value` フィールド内プレーンテキスト
  - `.cupola/steering/product.md` L3: `driving Claude Code + cc-sdd to automate design and implementation` — 英文散文の一部
- **Implications**: Markdown ファイルは `[cc-sdd](URL)` 形式、YAML はプレーンテキスト URL 形式を適用する

### YAML ファイルのリンク形式

- **Context**: YAML ファイルの `value` フィールドに markdown リンクを記述した場合の表示挙動
- **Findings**:
  - GitHub の Issue テンプレート YAML は `value` の内容を Markdown としてレンダリングするが、エディタや他ツールではプレーンテキスト扱いになる場合がある
  - Issue の要件定義（Requirement 2.2）ではプレーンテキスト形式での URL 括弧付き併記を許容している
  - YAML の `value` は複数行文字列（`|` ブロックスカラー）で記述されており、マークダウンリンク記法は構文的には有効
- **Implications**: GitHub レンダリングではマークダウンリンクが有効だが、要件との整合性から括弧付き URL 形式を採用する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Markdown リンク形式 | `[cc-sdd](URL)` | GitHub 上で即座にリンクとして表示される | YAML 等の非 Markdown コンテキストには不適 | Markdown ファイルに適用 |
| プレーンテキスト URL 形式 | `cc-sdd (URL)` | 全ファイル形式で安全に表示される | クリック不可（プレーンテキスト） | YAML ファイルに適用 |

## Design Decisions

### Decision: YAML ファイルへの適用形式

- **Context**: `.github/ISSUE_TEMPLATE/cupola-task.yml` の `value` フィールドは GitHub でマークダウンレンダリングされる
- **Alternatives Considered**:
  1. マークダウンリンク形式 `[cc-sdd](URL)` — GitHub レンダリング時に機能する
  2. プレーンテキスト形式 `cc-sdd (URL)` — 全コンテキストで安全
- **Selected Approach**: プレーンテキスト形式 `cc-sdd (https://github.com/gotalab/cc-sdd)` を採用
- **Rationale**: 要件 2.2 でプレーンテキスト形式を明示的に許容しており、実装の一貫性とリスク低減を優先する
- **Trade-offs**: GitHub での自動リンク化が得られないが、可読性と安全性のトレードオフ上は許容範囲
- **Follow-up**: 将来的にマークダウンリンク形式への切り替えが必要になる場合は要件を再確認すること

### Decision: 初出箇所のみへのリンク追加

- **Context**: 同一ファイル内に複数回 cc-sdd が登場する場合（product.md の L3 と L7）
- **Alternatives Considered**:
  1. 全出現箇所にリンクを追加 — 完全なリンク化
  2. 初出箇所のみにリンクを追加 — 標準的なドキュメント慣習
- **Selected Approach**: 初出箇所のみにリンクを追加（要件 3.3 に従う）
- **Rationale**: 一般的な技術ドキュメントの慣習に合致し、視覚的な冗長性を避けられる
- **Trade-offs**: 後続箇所ではリンクなし。ユーザーが下にスクロールした場合に再度リンクがないが許容範囲

## Risks & Mitigations

- リンク URL の誤り — 変更後に URL `https://github.com/gotalab/cc-sdd` が正確であることを確認すること
- 既存フォーマットの破壊 — 前後の文脈（太字・括弧・句読点）を維持する最小限の変更にとどめること

## References

- [cc-sdd GitHub リポジトリ](https://github.com/gotalab/cc-sdd) — リンク先の外部ツール
- [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) — CHANGELOG.md のフォーマット規約
