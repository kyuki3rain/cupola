# Research & Design Decisions

---
**Purpose**: `build_fixing_prompt` 分割における設計調査と決定根拠を記録する。
---

## Summary

- **Feature**: `issue-235` — DesignFixing / ImplementationFixing のプロンプト分割
- **Discovery Scope**: Extension（既存関数の分割・最適化）
- **Key Findings**:
  - `build_fixing_prompt` は `DesignFixing` と `ImplementationFixing` の両状態で共有されているが、修正対象（設計ドキュメント vs. 実装コード）が根本的に異なる
  - 現状の `GENERIC_QUALITY_CHECK_INSTRUCTION` はコード向けであり、設計ドキュメント修正エージェントには不適切
  - コミットメッセージプレフィックスが `fix:` に固定されており、ドキュメント修正には `docs:` が適切
  - マージコンフリクト処理・causes 解析・output-schema 生成は両種別で共通であり、内部ヘルパーとして再利用可能

## Research Log

### 既存 `build_fixing_prompt` の動作分析

- **Context**: `State::DesignFixing` と `State::ImplementationFixing` が同一関数を呼ぶことが Issue で指摘された
- **Sources Consulted**: `src/application/prompt.rs`（全文読取）
- **Findings**:
  - `build_fixing_prompt` は `issue_number`, `pr_number`, `language`, `causes: &[FixingProblemKind]`, `has_merge_conflict`, `default_branch` を引数に取る
  - プロンプト内で「Apply the necessary fixes to the code」という文言がハードコードされている（行 248）
  - コミットメッセージが `git commit -m "fix: address requested changes"` に固定されている（行 257）
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` は "Run quality checks described in AGENTS.md / CLAUDE.md before committing" という汎用文言で、設計ドキュメント修正時に clippy / test が走ると不適切
  - マージコンフリクトセクション・causes 解析・output-schema 生成の 3 ブロックは独立しており、再利用しやすい構造
- **Implications**:
  - `build_fixing_prompt` を `build_design_fixing_prompt` と `build_implementation_fixing_prompt` に分割する
  - 共通ロジック（コンフリクトセクション・output-schema・causes-to-instructions 変換）はプライベートヘルパー関数として抽出可能

### 既存テストの確認

- **Context**: 分割後に既存テストがどの程度壊れるかを評価
- **Findings**:
  - 現行テストはすべて `build_session_config` 経由でプロンプトを生成している
  - `design_fixing_returns_fixing_schema`（行 313）は `State::DesignFixing` + "automated review agent" という文言チェックのみ
  - `fixing_prompt_*` 系テストは `State::DesignFixing` を使っているが、`ImplementationFixing` 相当のテストは現状不足
  - 分割後は `DesignFixing` / `ImplementationFixing` それぞれに対応するテストを整備する必要がある
- **Implications**:
  - 既存の `design_fixing_returns_fixing_schema` などは引き続き通過するよう実装する
  - `DesignFixing` が "code" 関連文言を含まないことをアサートする新テストを追加

### 品質チェック指示の設計

- **Context**: 設計ドキュメント修正時に適切な品質チェックとは何かを決定する
- **Findings**:
  - 設計ドキュメント（requirements.md / design.md / tasks.md）の整合性チェックは、`GENERIC_QUALITY_CHECK_INSTRUCTION`（AGENTS.md / CLAUDE.md 参照）で十分かどうかを検討
  - 設計ドキュメント修正時の「品質チェック」は「要件・設計・タスクの整合性確認」であり、コードビルドとは独立
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` 自体は "quality checks described in AGENTS.md / CLAUDE.md" という形で汎用化されており、設計フェーズにも適用可能
  - ただし設計 fixing では「コードを修正する」という文言を除外し、ドキュメント修正の文脈を明確にすることが主目的
- **Implications**:
  - 品質チェック指示は `GENERIC_QUALITY_CHECK_INSTRUCTION` を両プロンプトで共用できる
  - 差分は主に「修正対象の説明文言」と「コミットメッセージプレフィックス」

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations |
|--------|-------------|-----------|---------------------|
| A: 完全分割（共通なし） | 2 関数を完全独立で実装 | シンプル・将来の独立進化が容易 | コードの重複（マージコンフリクト処理等） |
| B: ヘルパー抽出 + 2 関数 | 共通ロジックをプライベートヘルパーに抽出し 2 関数から呼ぶ | 重複最小・将来の差分追加も容易 | ヘルパーが増えるが許容範囲 |
| C: フラグ引数で分岐 | `build_fixing_prompt(kind: FixingKind, ...)` のように enum 引数を追加 | 関数 1 本で済む | 分岐が増え可読性低下・今回の目的に逆行 |

**選択**: Option B（ヘルパー抽出 + 2 関数）

## Design Decisions

### Decision: 共通ヘルパーの抽出範囲

- **Context**: マージコンフリクトセクション・causes-to-instructions 変換・output-schema セクションは両関数で同じロジックが必要
- **Alternatives Considered**:
  1. 完全重複 — コピーアンドペーストで 2 関数を独立実装
  2. ヘルパー関数として抽出 — `build_conflict_section`, `build_instructions_text`, `build_output_section` などをプライベート関数に切り出す
- **Selected Approach**: ヘルパー関数として抽出（Option 2）
- **Rationale**: 行数的に重複が大きく、将来 causes の種類が増えた際に片方だけ更新し忘れるリスクを防ぐ
- **Trade-offs**: ヘルパー関数が増えるが、プライベート関数なのでモジュール外の公開インタフェースには影響しない
- **Follow-up**: ヘルパー関数のテストは `build_design_fixing_prompt` / `build_implementation_fixing_prompt` のテストで間接的にカバーされる

### Decision: コミットメッセージプレフィックス

- **Context**: 設計ドキュメント修正のコミットには `docs:` プレフィックスが慣習的に正しい
- **Selected Approach**: `build_design_fixing_prompt` では `git commit -m "docs: address requested changes"`, `build_implementation_fixing_prompt` では `git commit -m "fix: address requested changes"` を指示する
- **Rationale**: Conventional Commits に準拠。設計ドキュメントの変更は `docs:` が適切
- **Trade-offs**: 変更なし（現行 `fix:` を `docs:` に変えるだけ）

### Decision: 「コードを修正する」文言の扱い

- **Context**: `DesignFixing` プロンプトで "Apply the necessary fixes to the code" は不適切
- **Selected Approach**: `build_design_fixing_prompt` では "Apply the necessary fixes to the design documents" のような文言に変更
- **Rationale**: エージェントが設計ドキュメントの修正に集中できるよう明示的な文脈を与える
- **Trade-offs**: なし

## Risks & Mitigations

- 既存テストが `DesignFixing` で "automated review agent" を確認しているが、文言変更で壊れる可能性 → 実装時にテスト内容を確認し必要に応じて更新
- `ImplementationFixing` 向けテストが現状不足 → タスクで追加テストを明示的に定義

## References

- `src/application/prompt.rs` — 現行の `build_fixing_prompt` 実装
- Conventional Commits specification — `docs:` プレフィックスの根拠
