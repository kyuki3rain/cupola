# Research & Design Decisions

---
**Purpose**: 設計決定の根拠と調査記録

---

## Summary
- **Feature**: `issue-358` — status コマンドへの ci_fix_count 表示追加
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - 変更箇所は `src/bootstrap/app.rs` の `handle_status()` 関数内、出力フォーマット 1 箇所のみ
  - 必要なデータ（`ci_fix_count`、`max_ci_fix_cycles`）はすでに関数に渡されている
  - DB マイグレーション・新設定・新ドメインロジックは一切不要

## Research Log

### 既存コードの調査

- **Context**: Issue に記載された変更対象コードの確認
- **Sources Consulted**: `src/bootstrap/app.rs`（直接読み込み）
- **Findings**:
  - `handle_status()` のシグネチャ: `async fn handle_status<W: std::io::Write>(out, pid_mgr, process_run_repo, repo, max_sessions, max_ci_fix_cycles: Option<u32>)`
  - l.836〜855: `for issue in &issues` ループ内で `pending_notification` を判定し、2 種類のフォーマットで出力
  - `issue.ci_fix_count: u32` はすでに `Issue` struct（`src/domain/issue.rs:14`）に存在
  - `max_ci_fix_cycles: Option<u32>` はすでに `handle_status` 引数として受け取られている
- **Implications**: 条件分岐のフォーマット文字列に `ci-fix: N` または `ci-fix: N/MAX` を追記するだけで実現できる

### 既存テストの調査

- **Context**: テスト変更の影響範囲を把握する
- **Sources Consulted**: `src/bootstrap/app.rs` テストセクション
- **Findings**:
  - `handle_status` のテストは `#[tokio::test]` ブロックとして多数存在（Task 3.1、3.2、5.3 など）
  - 出力文字列を `contains()` や `assert!` で検証するテストが中心
  - 既存テストは `ci-fix` 表示を期待しておらず、出力に `ci-fix:` が含まれても現在は通る（`contains` で部分一致のため）
  - フォーマット追加後、既存テストを `ci-fix: 0` 等が含まれることを確認するアサーションで補完する必要がある
- **Implications**: 既存テストの破壊リスクは低い。新規テストで `ci-fix:` フォーマットを明示的に検証する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| インライン修正 | `handle_status()` の `writeln!` フォーマット文字列を直接修正 | シンプル・変更最小 | なし | 採用 |
| ヘルパー関数抽出 | issue 行のフォーマットを別関数に切り出す | テスト容易性向上 | 過剰設計 | 不採用（本 Issue の範囲外） |

## Design Decisions

### Decision: インライン文字列修正のみで実現

- **Context**: 変更箇所が出力フォーマット 1 箇所のみであり、追加ロジックが不要
- **Alternatives Considered**:
  1. ヘルパー関数抽出 — テスト容易性は上がるが本 Issue の目的に対して過剰
  2. インライン文字列修正 — シンプルで保守しやすい
- **Selected Approach**: `for issue in &issues` ループ内の `writeln!` 呼び出し 2 箇所（pending/非 pending）を修正し、末尾に `ci-fix:` 情報を追記する
- **Rationale**: Issue が「変更点は出力フォーマット 1 箇所のみ」と明示している。DRY 原則よりも最小変更・明確な意図を優先する
- **Trade-offs**: フォーマット変更が 2 箇所に分散するが、どちらも同じ関数内の連続したコードであり管理容易
- **Follow-up**: テストで `ci-fix:` の有無・フォーマットを検証する

## Risks & Mitigations

- 既存テストが出力文字列の完全一致でなく `contains` 検証のため破壊リスクは低い — 変更後に全テスト通過を確認
- `ci_fix_count = 0` の場合も常に表示するため、ゼロ値の表示が意図通りかを設計・テストで明示する

## References

- Issue #358 本文
- `src/bootstrap/app.rs:836-856`（変更対象コード）
- `src/domain/issue.rs:14`（`ci_fix_count` フィールド定義）
