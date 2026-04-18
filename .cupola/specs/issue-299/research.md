# Research & Design Decisions

---
**Purpose**: 技術的な調査と設計判断の根拠を記録する。
**Feature**: `issue-299`
**Discovery Scope**: Extension（既存システムへの変更）

---

## Summary

- **Feature**: fixing プロンプトの `/cupola:fix` スキル委譲
- **Discovery Scope**: Extension
- **Key Findings**:
  - Design/Impl フェーズはすでにスキル委譲パターンを採用しており、Fixing フェーズも同パターンに統一可能
  - `/cupola:fix` スキルはすでに存在し、レビュースレッド対応・CI 失敗対応・コンフリクト解決の手順を網羅している
  - `has_merge_conflict` フラグに基づく明示的なセクションは、スキル側の自律検出により不要になる
  - `causes` パラメータは output-schema セクションの出力仕様生成に引き続き必要

## Research Log

### 既存プロンプトパターンの調査

- **Context**: Design/Impl フェーズがどのようにスキルを呼び出しているか確認
- **Sources Consulted**: `src/application/prompt.rs`
- **Findings**:
  - `build_design_prompt` は `Run /cupola:spec-design {feature_name}` を含む簡潔なプロンプトを生成
  - `build_implementation_prompt` は `Run /cupola:spec-impl {feature_name}` を含む簡潔なプロンプトを生成
  - 両者ともインライン手順を持たず、スキルに一任している
  - output-schema セクションは両者ともプロンプト側に記述されている
- **Implications**: Fixing フェーズも同パターンへ統一することで一貫性が生まれる

### `/cupola:fix` スキルの現状調査

- **Context**: スキルが fixing に必要な手順を網羅しているか確認
- **Sources Consulted**: `.claude/commands/cupola/fix.md`
- **Findings**:
  - Step 1: 入力ファイル（`review_threads.json`, `ci_errors.txt`）とコンフリクトマーカーを自律的に検出
  - Step 2: コンフリクトを最優先で解決（`git commit --no-edit`）
  - Step 3: レビューコメント対応
  - Step 4: CI 失敗修正
  - Step 5: 品質チェック（AGENTS.md / CLAUDE.md に従う）
  - Step 6: コミット・プッシュ（現状はコミットメッセージが `fix: address requested changes` で固定）
  - `design` / `impl` 引数（`$1`）を受け取る仕様だが、コミットメッセージの分岐がない
- **Implications**:
  - スキルを正本とするために、`design` 時は `docs:` プレフィックス、`impl` 時は `fix:` プレフィックスを付与する分岐を追加する必要がある
  - コンフリクト検出・対応はすでにスキル側で実装済みなので、prompt.rs の明示的な conflict section は不要

### `execute.rs` の呼び出しパターン調査

- **Context**: `build_session_config` の呼び出し箇所と `has_merge_conflict` の使われ方を確認
- **Sources Consulted**: `src/application/polling/execute.rs`
- **Findings**:
  - `has_merge_conflict` は `build_session_config` にのみ渡されており、他の用途はない
  - マージ処理自体（`worktree.merge`）は引き続き execute.rs で行われ、コンフリクトの有無をログ出力する
  - `has_merge_conflict` フラグを除去しても、マージ処理ロジックの変更は最小限
- **Implications**:
  - `build_session_config` シグネチャから `has_merge_conflict` と `default_branch` を除去できる
  - `execute.rs` の `has_merge_conflict` 変数宣言と代入も不要になる

### テストスイートの依存関係調査

- **Context**: 変更後に削除・更新が必要なテストを特定
- **Sources Consulted**: `src/application/prompt.rs` の `#[cfg(test)]` モジュール
- **Findings**:
  - `fixing_prompt_has_merge_conflict_true_inserts_section_at_top` — `## Merge Conflict Resolution Required` を検証 → 削除
  - `fixing_prompt_has_merge_conflict_false_is_unchanged` — conflict セクションがないことを検証 → 削除
  - `design_fixing_prompt_contains_docs_prefix` — `docs: address requested changes` を検証 → 更新（スキル委譲後は prompt にコミットメッセージ指示なし）
  - `implementation_fixing_prompt_contains_fix_prefix` — `fix: address requested changes` を検証 → 更新
  - `design_fixing_prompt_does_not_contain_code_wording` / `implementation_fixing_prompt_contains_code_wording` — インライン文言を検証 → 更新
  - `fixing_prompt_review_comments_only` / `fixing_prompt_ci_failure_only` — `review_threads.json` / `ci_errors.txt` 参照を検証 → スキルが対応するため prompt には含まれなくなる → 更新
  - output-schema セクション（`thread_id`, `{"threads": []}` 等）の検証は引き続き有効
- **Implications**: 多数のテストが削除・更新対象。新しいスキル委譲形式を検証するテストを追加する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| スキル委譲（採用） | prompt.rs を簡略化し `/cupola:fix` に一任 | 一貫性、保守性向上、単一責任 | スキル側の品質に依存する | Design/Impl フェーズと同じパターン |
| 現状維持 | インライン手順をそのまま維持 | 変更なし | 二重管理が継続、#297 未解消 | 却下 |
| prompt.rs に全統合 | スキルを廃止し prompt.rs に全手順記述 | 一元管理 | スキルのメリット喪失、コード肥大化 | 却下 |

## Design Decisions

### Decision: `causes` パラメータの維持

- **Context**: `has_merge_conflict` を除去するが `causes` は残すか？
- **Alternatives Considered**:
  1. `causes` も除去し、スキルが実行時にファイル存在確認で判断
  2. `causes` を維持し、output-schema セクションの動的生成に使用（採用）
- **Selected Approach**: `causes` パラメータを維持する
- **Rationale**: output-schema セクション（thread_id・resolved 等の出力仕様）はシステム側の責務であり、Claude Code がどの入力ファイルを処理したかに依存する。Cupola デーモンがすでに `causes` を把握しているため、プロンプト生成時に活用するのが合理的
- **Trade-offs**: `causes` 依存が残るが、output-schema の正確性を保証できる
- **Follow-up**: 将来的には output-schema 生成もスキル側に移す検討余地あり（#298 の方向性と一致）

### Decision: `has_merge_conflict` および `default_branch` の除去

- **Context**: prompt.rs から conflict section を除去すると、これらのパラメータが未使用になる
- **Alternatives Considered**:
  1. パラメータのみ残し将来の利用に備える
  2. 使用箇所がなくなったため即時除去（採用）
- **Selected Approach**: 両パラメータを `build_design_fixing_prompt`・`build_implementation_fixing_prompt`・`build_session_config` から除去する
- **Rationale**: 未使用パラメータは `clippy` に警告され、コードの意図を曖昧にする。スキルがコンフリクト検出を担うことが明確なので除去が適切
- **Trade-offs**: `execute.rs` の呼び出し側も更新が必要
- **Follow-up**: `execute.rs` の `has_merge_conflict` 変数（宣言・代入）も不要になるため合わせて削除

### Decision: コミットメッセージの動的生成を fix.md スキル側で実装

- **Context**: #297 でコミットメッセージの動的化が求められている
- **Alternatives Considered**:
  1. 固定文字列を維持（プレフィックスのみ分岐）
  2. AI に変更内容からコミットメッセージ本文を動的生成させる（採用）
- **Selected Approach**: `$1` 引数（`design` / `impl`）でプレフィックスを決定し、メッセージ本文は変更内容に基づき AI が生成
- **Rationale**: 変更内容を最もよく知っているのは実行時の AI であり、固定文字列より意味のあるコミット履歴を生成できる
- **Trade-offs**: テストでコミットメッセージの固定文字列アサーションが使えなくなる（prompt.rs にコミットメッセージ指示がなくなるため）

## Risks & Mitigations

- スキルの品質低下リスク — スキルを正本として整備し、CI で品質チェックを維持する
- テスト削除による検証ギャップ — 旧テストを削除する代わりに、スキル委譲形式の新テストを追加する
- `execute.rs` の変更漏れ — `build_session_config` シグネチャ変更はコンパイルエラーで検出可能

## References

- `src/application/prompt.rs` — 変更対象の fixing プロンプト生成ロジック
- `.claude/commands/cupola/fix.md` — 正本となる `/cupola:fix` スキル
- `src/application/polling/execute.rs` — `build_session_config` 呼び出し箇所（シグネチャ変更の影響を受ける）
