# Research & Design Decisions

---
**Purpose**: `build_fixing_prompt` 改善のための調査記録

---

## Summary
- **Feature**: `improve-build-fixing-prompt`
- **Discovery Scope**: Extension（既存関数の修正）
- **Key Findings**:
  - `build_fixing_prompt` は `_issue_number` / `_pr_number` を受け取っているが未使用。パラメータ名のアンダースコアを除去してプロンプト文字列に埋め込むだけで対応可能
  - output セクションの条件分岐は `causes` スライスに対する `contains` 判定で実装可能。`ReviewComments` の有無でテンプレート文字列を切り替える
  - `git add -A` の置き換えは文字列テンプレートの変更のみ。外部依存・DB・アダプタへの影響なし

## Research Log

### 既存コードの構造分析

- **Context**: `src/application/prompt.rs` の `build_fixing_prompt` を理解するため
- **Findings**:
  - `build_fixing_prompt(issue_number, pr, &config.language, fixing_causes)` として呼び出される
  - `DesignFixing` / `ImplementationFixing` の両状態で共通利用
  - `fixing_causes: &[FixingProblemKind]` は既に引数として渡されており、`ReviewComments` / `CiFailure` / `Conflict` の3種
  - `FIXING_SCHEMA` は `{"threads": [...]}` を必須フィールドとして定義している
  - output セクションはレビュースレッドのoutputを無条件に要求しているが、CI/コンフリクト修正時はスレッドが存在しない
- **Implications**:
  - `FIXING_SCHEMA` は変更不要。`threads` は空配列 `[]` でも有効
  - output 指示を `causes` に応じて分岐させることで、スキーマとの整合性を保ちながら誤解を排除できる

### git add の安全性

- **Context**: `git add -A` の代替案を検討
- **Findings**:
  - design prompt は `git add .cupola/specs/ .cupola/steering/` と特定ディレクトリを指定
  - implementation prompt は `git push` のみ（commit指示なし）
  - fixing prompt は任意のファイルを修正するため、特定パスの指定は困難
  - `git diff --name-only` でステージング前に変更ファイルを確認させるアプローチが安全
- **Implications**:
  - `git add <変更したファイル>` の形で個別ファイル指定を促す
  - または `git diff --name-only` で確認後にステージングするよう明示
  - 一時ファイル・デバッグログの混入リスクを排除

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| テンプレート文字列条件分岐 | `causes` チェックで output セクションの文字列を切り替え | シンプル、既存パターンと統一 | なし | 採用 |
| 別関数分割 | ReviewComments用/CI用に関数を分離 | 関数が単純になる | 重複コード増加、`build_session_config` の変更が必要 | 不採用 |

## Design Decisions

### Decision: output指示の分岐方法

- **Context**: `ReviewComments` の有無でoutput指示を変える必要がある
- **Alternatives Considered**:
  1. `causes` チェックで文字列を条件分岐 — 最小変更
  2. `build_fixing_prompt` を2つの関数に分割
- **Selected Approach**: `causes.contains(&FixingProblemKind::ReviewComments)` の結果でoutputセクション文字列を選択し、`format!` に埋め込む
- **Rationale**: 既存の `instructions` 構築ロジックと同じパターン。追加の抽象化不要
- **Trade-offs**: 関数が若干長くなるが、可読性は維持される
- **Follow-up**: テストで `{"threads": []}` の有無を確認

### Decision: git add の置き換え

- **Context**: `git add -A` は意図しないファイルをステージングするリスクがある
- **Alternatives Considered**:
  1. `git add <changed_files>` と説明文で個別指定を促す
  2. `git diff --name-only` で確認後にステージング
- **Selected Approach**: `git add` に続けて「修正したファイルのみをステージングすること」と明示し、`git diff --name-only` を使った確認手順を案内
- **Rationale**: Claude Code に具体的な手順を提示することで誤った操作を防ぐ
- **Trade-offs**: プロンプトがやや長くなる

## Risks & Mitigations

- 既存テストの assertion 更新漏れ → テスト更新を Requirement 4 として明示的に要件化済み
- `FIXING_SCHEMA` の `threads` 必須制約との不整合 → `{"threads": []}` を返すよう明示することで解消

## References
- `src/application/prompt.rs` — 修正対象ファイル
- `src/domain/fixing_problem_kind.rs` — `FixingProblemKind` 定義
