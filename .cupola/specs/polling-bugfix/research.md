# 調査・設計判断の記録

## Summary
- **Feature**: `polling-bugfix`
- **Discovery Scope**: Extension（既存システムのバグ修正）
- **Key Findings**:
  - Step 1 の Issue close 検知が Step 4 の PR merge 検知より先に実行されるため、Closes #N による自動 close が cancelled として処理される
  - review_waiting 状態で Issue close を検知した場合に PR merge を先行チェックすることで、既存のステートマシンを変更せず修正可能
  - feature_name は Issue エンティティに nullable カラムとして追加し、設計フェーズの output-schema で受け取る設計が最もシンプル

## Design Decisions

### Decision: Issue close 検知時の merge 先行チェック

- **Context**: polling Step 1 で Issue close を検知した時点で IssueClosed イベントを即座に発行するため、PR merge による Closes 自動 close と区別できない
- **Alternatives Considered**:
  1. ステートマシンの遷移ルールを変更し、review_waiting + IssueClosed → completed にする
  2. Step 1 で review_waiting 状態の Issue close 検知時に PR merge を先行チェックする
  3. PR body から Closes #N を除外する
- **Selected Approach**: Option 2 — Step 1 での merge 先行チェック
- **Rationale**: ステートマシンは純粋関数として維持。PR body の制約は不自然。polling_use_case の Step 1 内で review_waiting 状態の場合のみ追加チェックを行うのが最小変更
- **Trade-offs**: polling サイクルごとに review_waiting Issue に対して追加の API 呼び出し（is_pr_merged）が発生するが、Step 4 でも同じ呼び出しを行うため実質的な増加は Issue close 時のみ

### Decision: feature_name の保存場所

- **Context**: 設計フェーズで Claude Code が決定する feature name を実装フェーズで参照する必要がある
- **Alternatives Considered**:
  1. Issue エンティティに feature_name カラムを追加
  2. 別テーブル（issue_metadata）に保存
  3. worktree 内の spec.json を直接読み取る
- **Selected Approach**: Option 1 — Issue エンティティにカラム追加
- **Rationale**: 最もシンプル。既存の Issue update フローで自然に保存・参照可能。nullable TEXT で後方互換性も維持
