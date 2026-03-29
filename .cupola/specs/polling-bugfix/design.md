# Design Document: polling-bugfix

## Overview

**Purpose**: Cupola の polling ループにおける 2 つのバグを修正する。(1) PR merge 時の `Closes #N` による Issue 自動 close が cancelled として処理される問題、(2) 実装フェーズで正しい feature name を特定できない問題。
**Impact**: polling_use_case.rs の Step 1 ロジック変更、output-schema の拡張、Issue エンティティへのカラム追加。

### Goals
- review_waiting 状態での Issue close 検知時に PR merge を先行確認し、merge 済みなら completed に遷移する
- 設計フェーズの output-schema に feature_name を追加し、実装プロンプトに埋め込む

### Non-Goals
- ステートマシン（StateMachine::transition）の変更
- PR body から Closes #N を除外する制約の導入

## Architecture

### Existing Architecture Analysis

変更対象は以下の 3 レイヤーにまたがる:

- **domain**: Issue エンティティに `feature_name` カラム追加
- **application**: polling_use_case.rs（Step 1 ロジック）、prompt.rs（output-schema + プロンプト）、io.rs（PrCreationOutput）
- **adapter**: sqlite_issue_repository.rs / sqlite_connection.rs（スキーマ）

ステートマシン（domain/state_machine.rs）は変更しない。遷移ルール自体は正しく、問題は polling_use_case がイベントを生成するタイミングにある。

## Requirements Traceability

| Requirement | Summary | Components | 変更内容 |
|-------------|---------|------------|---------|
| 1.1 | impl review_waiting + Issue close → merge 先行チェック | polling_use_case.rs Step 1 | close 検知時に is_pr_merged を呼び出し |
| 1.2 | design review_waiting + Issue close → merge 先行チェック | polling_use_case.rs Step 1 | 同上（design_pr_number を使用） |
| 1.3 | PR 未 merge 時は従来通り cancelled | polling_use_case.rs Step 1 | 既存動作を維持 |
| 1.4 | merge + close 同時検知 → completed 優先 | polling_use_case.rs Step 1 | merge 先行チェックで自然に実現 |
| 1.5 | review_waiting 以外は従来通り | polling_use_case.rs Step 1 | 条件分岐で review_waiting のみ対象 |
| 2.1 | output-schema に feature_name 追加 | prompt.rs | PR_CREATION_SCHEMA 拡張 |
| 2.2 | 設計プロンプトに feature_name 出力指示追加 | prompt.rs | build_design_prompt 修正 |
| 2.3 | feature_name を DB に記録 | polling_use_case.rs, io.rs | create_pr_from_output で保存 |
| 2.4 | 実装プロンプトに feature_name 埋め込み | prompt.rs | build_implementation_prompt 修正 |
| 2.5 | feature_name 抽出失敗時のフォールバック | polling_use_case.rs | None の場合は従来の ls 指示 |
| 2.6 | issues テーブルに feature_name カラム追加 | issue.rs, sqlite_*.rs | ALTER TABLE 相当 |

## Components and Interfaces

### 変更 1: polling_use_case.rs — Step 1 の Issue close 検知ロジック

**現在の動作**:
```
Issue close 検知 → IssueClosed イベント発行
```

**修正後の動作**:
```
Issue close 検知
  → review_waiting 状態？
    → Yes: PR merge を確認（is_pr_merged）
      → merge 済み: PrMerged イベント発行（IssueClosed ではなく）
      → 未 merge: IssueClosed イベント発行
    → No: IssueClosed イベント発行（従来通り）
```

PR 番号の取得:
- `DesignReviewWaiting` → `issue.design_pr_number`
- `ImplementationReviewWaiting` → `issue.impl_pr_number`

### 変更 2: prompt.rs — output-schema と プロンプト

**PR_CREATION_SCHEMA の変更**:
```json
{
  "type": "object",
  "properties": {
    "pr_title": { "type": "string" },
    "pr_body": { "type": "string" },
    "feature_name": { "type": "string", "description": "cc-sdd の feature name（.cupola/specs/ 配下のディレクトリ名）" }
  },
  "required": ["pr_title", "pr_body"]
}
```
`feature_name` は required ではない（フォールバック対応）。

**build_design_prompt の変更**:
output-schema 出力指示に以下を追加:
```
- feature_name: cc-sdd の feature name（spec-init で生成したディレクトリ名）
```

**build_implementation_prompt の変更**:
シグネチャに `feature_name: Option<&str>` を追加。

- `Some(name)` の場合: `/kiro:spec-impl {name}` を明示的に指示
- `None` の場合: 従来の `ls .cupola/specs/` + phase フィルタ指示（フォールバック）

**build_session_config の変更**:
シグネチャに `feature_name: Option<&str>` を追加し、`ImplementationRunning` 時に `build_implementation_prompt` に渡す。

### 変更 3: io.rs — PrCreationOutput

```rust
#[derive(Debug, Deserialize)]
pub struct PrCreationOutput {
    pub pr_title: Option<String>,
    pub pr_body: Option<String>,
    pub feature_name: Option<String>,  // 追加
}
```

### 変更 4: domain/issue.rs — Issue エンティティ

```rust
pub struct Issue {
    // ... 既存フィールド
    pub feature_name: Option<String>,  // 追加
}
```

### 変更 5: adapter/outbound/sqlite_*.rs — スキーマとリポジトリ

**sqlite_connection.rs**: issues テーブルに `feature_name TEXT` カラムを追加（CREATE TABLE IF NOT EXISTS で定義）。既存 DB の場合は起動時に `ALTER TABLE issues ADD COLUMN feature_name TEXT` を安全に実行（カラムが既に存在する場合はスキップ）。

**sqlite_issue_repository.rs**: SELECT / INSERT / UPDATE クエリに feature_name を追加。

### 変更 6: polling_use_case.rs — create_pr_from_output での feature_name 保存

設計フェーズ（DesignRunning）の正常終了後、output から feature_name を抽出し Issue レコードに保存する。

```
output.feature_name が Some → issue.feature_name = Some(name)、DB 更新
output.feature_name が None → スキップ（フォールバック）
```

## Data Models

### issues テーブルの変更

```sql
-- 新規作成時（CREATE TABLE IF NOT EXISTS に含める）
feature_name TEXT

-- 既存 DB のマイグレーション（起動時に実行）
ALTER TABLE issues ADD COLUMN feature_name TEXT;
```

## Error Handling

- PR merge チェックの API エラー: ログ記録し、安全側に倒して IssueClosed イベントを発行（従来動作）
- feature_name パース失敗: None として扱い、フォールバックプロンプトを使用
- ALTER TABLE の重複実行: SQLite は `ADD COLUMN` が既存カラムに対してエラーを返すため、エラーを無視する

## Testing Strategy

### Unit Tests
- Step 1 の review_waiting + Issue close → merge 先行チェックのロジック検証
- PrCreationOutput の feature_name パーステスト
- build_implementation_prompt の feature_name 有無による分岐テスト
- build_session_config の feature_name 引き渡しテスト

### Integration Tests
- review_waiting 状態で Issue close + PR merge 済み → completed 遷移の統合テスト
- review_waiting 状態で Issue close + PR 未 merge → cancelled 遷移の統合テスト
