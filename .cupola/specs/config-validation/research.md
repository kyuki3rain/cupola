# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary
- **Feature**: `config-validation`
- **Discovery Scope**: Extension（既存システムへの追加）
- **Key Findings**:
  - `Config` は `src/domain/config.rs` に定義された値オブジェクトであり、`validate()` は pure な同期メソッドとして実装されている
  - 既存の `max_concurrent_sessions` チェックは早期リターン方式で実装されており、新規チェックも同方式で統一する
  - `stall_timeout_secs` と `polling_interval_secs` の相関チェックは、両フィールドの絶対下限チェックを通過した後にのみ実施する（Requirement 4.4）

## Research Log

### 既存 Config 実装の調査
- **Context**: 追加すべきバリデーションが既存実装のどのパターンに合うかを確認
- **Sources Consulted**: `src/domain/config.rs`（直接読取）
- **Findings**:
  - `Config` struct は `String` フィールド（owner, repo, default_branch, language, model）と `u64` フィールド（polling_interval_secs, stall_timeout_secs）を持つ
  - `validate(&self) -> Result<(), String>` は `Ok(())` を返すシンプルな pure 関数
  - 既存テストは `#[cfg(test)] mod tests` ブロックに 3 件含まれる
  - デフォルト値: `polling_interval_secs = 60`, `stall_timeout_secs = 1800`（共に新下限を満たす）
- **Implications**: デフォルト値は全て新バリデーションを通過するため、既存のデフォルト動作への影響なし

### バリデーション順序の設計
- **Context**: 複数チェックの評価順序と相関チェックの前提条件
- **Findings**:
  - Requirement 4.4 により、`stall_timeout_secs <= polling_interval_secs` の比較チェックは両絶対下限違反がない場合のみ実行
  - 文字列フィールドのチェックは `polling_interval_secs` / `stall_timeout_secs` の前に実施（設定ファイルの記述順）
  - 各チェックは最初のエラーで早期リターンする（Requirement 1.3）
- **Implications**: チェックの評価順序が明示的な設計上の決定事項となる

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存 if-let 方式の拡張 | 既存の早期リターンパターンに新チェックを追加 | シンプル、最小変更 | なし | 採用 |
| エラーの収集（Vec） | 全バリデーションエラーを収集して一度に返す | ユーザーが全エラーを一度に確認できる | 相関チェックの依存関係管理が複雑になる | 今回は採用しない（Issueの仕様と一致しない） |

## Design Decisions

### Decision: 早期リターン方式の継続
- **Context**: 複数バリデーションエラーが存在する場合の返し方
- **Alternatives Considered**:
  1. 早期リターン — 最初に検出したエラーを返す
  2. エラー収集 — 全エラーを Vec に集めて返す
- **Selected Approach**: 早期リターン（既存パターンを踏襲）
- **Rationale**: Requirement 1.3 で「最初に検出したフィールドのエラーで早期リターン」と明示されている。既存実装との一貫性を維持
- **Trade-offs**: ユーザーは1回の起動で1つのエラーしか確認できないが、設定ミスは通常1箇所であるため実用上の問題は少ない
- **Follow-up**: エラーメッセージの文言は英語（Requirement 6.3）

### Decision: 相関チェックの前提条件ガード
- **Context**: `stall_timeout_secs <= polling_interval_secs` チェックは、両フィールドが不正値の場合にも実行すべきか
- **Alternatives Considered**:
  1. 常に相関チェックを実施
  2. 両絶対下限を満たした場合のみ相関チェックを実施
- **Selected Approach**: 絶対下限チェック通過後に相関チェック
- **Rationale**: Requirement 4.4 で明示。絶対下限違反がある状態での比較は misleading なエラーメッセージになる
- **Trade-offs**: チェック順序が厳密に定まる

### Decision: default_branch / language / model の空文字チェック追加
- **Context**: Issue では owner/repo に加えて default_branch/language/model の空文字チェックも要求されているが、requirements.md の Requirement 1 は owner/repo のみをスコープとしている
- **Alternatives Considered**:
  1. requirements.md のスコープ（owner/repo のみ）に限定する
  2. Issue の意図を汲み取り全 5 フィールドを対象にする
- **Selected Approach**: Issue の意図に従い全 5 フィールドをチェック対象とする（Requirement 1 の拡張解釈）
- **Rationale**: requirements.md の Requirement 1 AC は owner/repo のみだが、Issue の追加バリデーション表では同じ理由（起動時エラー防止）で 5 フィールドが並列に列挙されている。設計段階でスコープを揃えることで実装の手戻りを防ぐ
- **Follow-up**: タスク生成時に AC を拡張するか、または要件文書を更新すること

## Risks & Mitigations
- デフォルト値が新バリデーションを通過しないリスク → 調査済み。全デフォルト値は新下限を満たす（polling=60≥10, stall=1800≥60, stall>polling）
- 既存テストのリグレッション → テストで使用している "o"/"r"/"main" はすべて非空文字列のためリグレッションなし

## References
- `src/domain/config.rs` — 既存 Config 実装（直接調査）
- Issue #130 — 追加バリデーション仕様の出典
