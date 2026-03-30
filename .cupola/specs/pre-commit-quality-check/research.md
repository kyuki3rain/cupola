# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary
- **Feature**: `pre-commit-quality-check`
- **Discovery Scope**: Simple Addition（既存関数へのプロンプト文字列追加）
- **Key Findings**:
  - 変更対象は `src/application/prompt.rs` 内の3つのプライベート関数のみ
  - 追加するのはプロンプト文字列のテキストであり、Rustのロジック変更は不要
  - 既存テストはプロンプト内容の存在確認をしており、新規テストで品質チェック文言の存在を検証できる

## Research Log

### 既存プロンプト関数の構造分析

- **Context**: 3つのプロンプト生成関数（`build_design_prompt`、`build_implementation_prompt`、`build_fixing_prompt`）の現状把握
- **Sources Consulted**: `src/application/prompt.rs`（直接コード参照）
- **Findings**:
  - `build_design_prompt`: 手順1〜6の手順書形式。手順6が `git commit / git push`。品質チェックは手順5と6の間に挿入する
  - `build_implementation_prompt`: `feature_name` の有無で分岐。最終ステップが `git push`。品質チェックはpushステップの直前に挿入する
  - `build_fixing_prompt`: 手順1〜3。手順3が `git commit -m "fix: address review comments" / git push`。品質チェックは手順3の直前に挿入する
- **Implications**: 各関数のformat文字列に品質チェックセクションを追加するだけで要件を満たせる

### cargo品質チェックコマンドの確認

- **Context**: プロンプトに記述すべきコマンドの正確な形式の確認
- **Sources Consulted**: `.cupola/steering/tech.md`、`.claude/rules/rust-hooks.md`
- **Findings**:
  - `cargo fmt` — コード整形
  - `cargo clippy -- -D warnings` — 全警告をエラーとして静的解析
  - `cargo test` — 全テスト実行
  - tech.mdのDevelopment Standardsにも同様の記述あり（一貫性確認済み）
- **Implications**: コマンド形式は既存のプロジェクト標準と完全に一致しており、追加のコマンド設計は不要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| プロンプト文字列への直接追加 | 各関数のformat文字列にテキストブロックを追記 | 最小変更、副作用なし、既存テストへの影響最小 | なし | 採用 |
| 共通ヘルパー関数の抽出 | 品質チェック文字列を返す共通関数を定義 | DRY原則 | 今回は3関数だが抽象化のコストが価値を上回る | 不採用（YAGNI） |

## Design Decisions

### Decision: 品質チェック手順の挿入位置

- **Context**: 品質チェックはcommit直前に実行する必要がある
- **Alternatives Considered**:
  1. commitと同じ手順番号内に記述 — 手順が長くなり読みにくい
  2. 独立した手順番号として挿入（commitの直前） — 明確で構造的
- **Selected Approach**: commitの直前に独立した手順番号として挿入
- **Rationale**: Claude Codeが手順を順番に読む際、commitの前に必ず品質チェックを実行させるため、独立した手順番号が最も明確
- **Trade-offs**: 手順番号が1つ増えるが、可読性と明確さが向上する

### Decision: 共通関数の不採用

- **Context**: 3つの関数に同じテキストを追加する際の重複排除
- **Alternatives Considered**:
  1. 共通ヘルパー関数 `quality_check_instructions()` を定義
  2. 各関数に直接テキストを追加
- **Selected Approach**: 各関数に直接テキストを追加
- **Rationale**: 各プロンプトの文脈が異なり（設計/実装/修正）、将来的に個別カスタマイズが必要になる可能性がある。現時点では抽象化のコストが価値を上回る
- **Trade-offs**: テキストに重複が生じるが、各プロンプトの独立性が保たれる

## Risks & Mitigations

- 既存テストへの影響 — 既存テストはプロンプト内の特定文字列の存在を確認するのみであり、文字列追加による影響はない
- 手順番号のずれ — `build_implementation_prompt` では `push_step` 変数で手順番号を動的に生成しているため、新規テキスト追加後も番号が適切に管理される設計を確認する

## References

- `src/application/prompt.rs` — 変更対象ファイル
- `.cupola/steering/tech.md` — cargo コマンドの標準形式
