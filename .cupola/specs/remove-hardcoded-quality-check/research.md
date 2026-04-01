# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary
- **Feature**: `remove-hardcoded-quality-check`
- **Discovery Scope**: Extension（既存システムへの変更）
- **Key Findings**:
  - 変更対象は `src/application/prompt.rs` の 1 ファイルのみ。`build_design_prompt`・`build_implementation_prompt`・`build_fixing_prompt` の 3 関数内にハードコードされた品質チェック手順を置き換える。
  - Claude Code は起動時にプロジェクトルートの AGENTS.md・CLAUDE.md を自動読み込みするため、プロンプト側で「これらのファイルに従え」と指示するだけで汎用的な品質チェックが実現できる。
  - テストファイル末尾に `cargo fmt` / `cargo clippy` / `cargo test` の存在をアサートする 4 つのテスト（`design_prompt_contains_quality_check`、`implementation_prompt_contains_quality_check`、`implementation_prompt_without_feature_name_contains_quality_check`、`fixing_prompt_contains_quality_check`）があり、これらを新仕様に合わせて更新する必要がある。

## Research Log

### プロンプト関数の現状調査
- **Context**: 変更対象となる 3 関数の品質チェック記述を特定するため
- **Sources Consulted**: `src/application/prompt.rs`（行 98–103、157–162、226–231）
- **Findings**:
  - `build_design_prompt`（行 98–103）: ステップ 6 として `cargo fmt`・`cargo clippy -- -D warnings`・`cargo test` をリスト形式で記載
  - `build_implementation_prompt`（行 157–162）: 変数 `quality_check_step` を使い同一コマンドを記載
  - `build_fixing_prompt`（行 226–231）: ステップ 3 として同一コマンドを記載
- **Implications**: 3 ヶ所の文字列リテラルを汎用指示文字列に置換すればよい。ロジック変更は不要。

### テストの現状調査
- **Context**: 変更に伴って修正が必要なテストを特定するため
- **Sources Consulted**: `src/application/prompt.rs`（行 462–539）
- **Findings**:
  - `design_prompt_contains_quality_check`（行 462–477）: `cargo fmt`・`cargo clippy -- -D warnings`・`cargo test` の存在をアサート
  - `implementation_prompt_contains_quality_check`（行 479–502）: 同上（feature_name あり）
  - `implementation_prompt_without_feature_name_contains_quality_check`（行 504–521）: 同上（feature_name なし）
  - `fixing_prompt_contains_quality_check`（行 523–539）: 同上
  - これら 4 テストは新仕様で失敗するため、汎用指示文字列の存在を検証するテストへの置き換えが必要
- **Implications**: テスト名を変更し、アサート内容を「汎用品質チェック指示の存在確認」に変更する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 文字列リテラル直接置換 | 各プロンプト関数内のハードコード文字列を汎用指示に差し替え | 最小変更、既存アーキテクチャ維持 | 変更箇所が 3 ヶ所に分散 | 本機能の規模に最適 |
| 品質チェック指示を定数化 | 汎用品質チェック文字列を定数として切り出す | DRY 原則、一元管理 | 軽微なオーバーエンジニアリングの恐れ | 変更が将来も続くなら検討余地あり |

## Design Decisions

### Decision: 汎用品質チェック指示の文言
- **Context**: 置き換え後に使用する汎用指示文字列をどうするか
- **Alternatives Considered**:
  1. 短い一文: "AGENTS.md / CLAUDE.md の指示に従い品質チェックを行うこと"
  2. Issue 要求事項の文言: "commit 前に AGENTS.md / CLAUDE.md に記載された品質チェックを実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。"
- **Selected Approach**: Issue 要求事項の文言（Option 2）をそのまま採用
- **Rationale**: 要件仕様との一致を保ち、実装者の解釈ブレを防ぐ
- **Trade-offs**: 文言が長くなるが、Claude Code への指示明確性が上がる
- **Follow-up**: 実装後、テスト内でアサートする文字列との整合を確認すること

### Decision: 汎用指示を定数として切り出すか
- **Context**: 3 関数に同一文字列を埋め込むか、定数として共有するか
- **Alternatives Considered**:
  1. 各関数に直接埋め込む（インライン）
  2. モジュールレベルの `const` として定義し参照
- **Selected Approach**: `const` 定数として切り出す
- **Rationale**: 3 関数で完全に同一の文字列を使用するため DRY 原則に従う。将来の文言変更も 1 箇所で完結する。
- **Trade-offs**: 定数名の命名が必要になるが軽微
- **Follow-up**: 定数名は `GENERIC_QUALITY_CHECK_INSTRUCTION` 相当が望ましい

## Risks & Mitigations
- 旧テスト（cargo コマンド存在確認）の削除漏れ — 変更後に `cargo test` を実行してすべてパスすることで確認
- 汎用指示文字列の typo — テスト内アサートで文言の一部を検証することで検出

## References
- `src/application/prompt.rs` — 変更対象ファイル
- Issue #73 — 本変更の要件定義元
