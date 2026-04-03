# 調査・設計決定ログ

---
**Purpose**: ディスカバリーの発見、アーキテクチャ調査、技術的設計の根拠を記録する。
---

## Summary
- **Feature**: `ci-fix-count-limit`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `Issue` エンティティに `ci_fix_count: u32` が未存在のため、新規フィールドとして追加する必要がある
  - `step4_pr_monitoring` は既に `FixingProblemKind`（CiFailure / Conflict / ReviewComments）を収集する仕組みを持ち、拡張ポイントが明確
  - SQLite スキーマは `ALTER TABLE ADD COLUMN` 方式でのインクリメンタルマイグレーションを採用しており、既存パターンに従えばよい

## Research Log

### 既存 Issue エンティティの調査
- **Context**: `ci_fix_count` フィールドが既に存在するか確認
- **Findings**:
  - `src/domain/issue.rs` に `ci_fix_count` は存在しない
  - 現在は `fixing_causes: Vec<FixingProblemKind>` で「何が問題か」を管理するが、「何回試みたか」は管理していない
  - `retry_count: u32` は ProcessFailed 起因のリトライ回数に特化しており、CI/Conflict 起因の修正試行には用途が異なる
- **Implications**: `ci_fix_count` を新規フィールドとして `Issue` に追加する。`retry_count` との混同を避けるため、明確に別フィールドとする

### step4_pr_monitoring の調査
- **Context**: カウンタのインクリメント・リセット・上限チェックロジックをどこに挿入するか
- **Findings**:
  - `src/application/polling_use_case.rs` の `step4_pr_monitoring` が原因収集後に `Event::FixingRequired` を emit する
  - `fixing_causes` が空の場合、現在は `FixingRequired` を emit しない（正しい動作）
  - `FixingRequired` emit 直前が最適な挿入ポイント
- **Implications**: `causes` の内容に応じてカウンタ操作を行い、上限到達時は emit をスキップしてコメント投稿する

### transition_use_case の調査
- **Context**: フェーズ変更時（Design → Implementation）のカウンタリセット場所
- **Findings**:
  - `src/application/transition_use_case.rs` の `execute_side_effects` でフェーズ変更の副作用を管理
  - `DesignPrMerged` イベントで `ImplementationRunning` に遷移するタイミングがフェーズ変更の境界
- **Implications**: `execute_side_effects` 内の `DesignPrMerged` ブランチで `ci_fix_count` を 0 にリセットする

### SQLite スキーマ・マイグレーション調査
- **Context**: `ci_fix_count` カラム追加方法
- **Findings**:
  - `src/adapter/outbound/sqlite_connection.rs` で `ALTER TABLE ADD COLUMN IF NOT EXISTS` パターンを使用
  - `fixing_causes TEXT NOT NULL DEFAULT '[]'` が最後に追加されたカラム
  - `CREATE TABLE` 文自体は変更せず、マイグレーションブロックに追加するのが既存パターン
- **Implications**: マイグレーションに `ci_fix_count INTEGER NOT NULL DEFAULT 0` を追加するのみでよい

### Config への max_ci_fix_cycles 追加調査
- **Context**: 設定値の追加方法
- **Findings**:
  - `src/domain/config.rs` で `Config` 構造体を定義
  - `src/bootstrap/config_loader.rs` で TOML をパースして `Config` を構築
  - `max_retries: u32` が類似パターンとして存在（デフォルト 3）
- **Implications**: `max_ci_fix_cycles: u32` を `max_retries` と同様のパターンで追加する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| step4 内でカウンタ管理 | step4_pr_monitoring でインクリメント・リセット・上限チェックをすべて行う | ロジックが一か所に集中、追いやすい | step4 が少し複雑になる | **採用** — emit の直前での判断が自然 |
| StateMachine にカウンタロジック追加 | StateMachine.transition() でカウンタ判断 | 状態遷移と一体化 | domain に application ロジックが入り込む | 却下 — Clean Architecture 違反 |
| 別 UseCase として分離 | CiFixCountUseCase を新設 | 単一責任 | 過設計、既存 step4 との連携が複雑 | 却下 — この規模には過剰 |

## Design Decisions

### Decision: カウンタリセット条件
- **Context**: ReviewComments と CiFailure が同時に存在する場合のリセット判断
- **Alternatives Considered**:
  1. CiFailure/Conflict が 1 つでもあればインクリメント
  2. ReviewComments が含まれる場合はリセット優先
- **Selected Approach**: ReviewComments を含む場合は常にリセット（オプション 2）
- **Rationale**: ReviewComments によってコードが書き換わると CI 状況も変わりうるため、リセットが妥当。Issue #147 の設計方針と一致
- **Trade-offs**: CI が通らないまま ReviewComments 修正だけで上限リセットされる可能性があるが、人間のレビュー介入があることを重視

### Decision: 上限到達時の通知タイミング
- **Context**: コメント投稿を毎回行うか、1 回だけにするか
- **Alternatives Considered**:
  1. 上限到達ポーリングサイクルごとに毎回コメント
  2. 1 回だけ（to avoid spam）
- **Selected Approach**: `ci_fix_count == max_ci_fix_cycles` の瞬間（ちょうど到達時）に 1 回のみ投稿
- **Rationale**: GitHub Issues のコメントスパムを防ぐ。`ci_fix_count` が変化しなくなるため、「到達した瞬間」を検出可能
- **Trade-offs**: ポーリング再起動後に再度コメントが投稿される可能性があるが、許容範囲

### Decision: フェーズ変更のリセットポイント
- **Context**: Design → Implementation 遷移時のリセット実装場所
- **Alternatives Considered**:
  1. `transition_use_case.execute_side_effects` の `DesignPrMerged` ブランチ
  2. `step4_pr_monitoring` で状態を見てリセット
- **Selected Approach**: `execute_side_effects` の `DesignPrMerged` ブランチ
- **Rationale**: フェーズ変更の副作用は `execute_side_effects` に集約されており、既存パターンに従う

## Risks & Mitigations
- **再起動後の重複コメント**: `ci_fix_count == max` の条件でコメントするため、再起動後に再度コメントが投稿される可能性 → 許容（実用上問題ない頻度）
- **マイグレーション漏れ**: `ci_fix_count` カラムなしに読み取るとエラー → `DEFAULT 0` のマイグレーションで解決
- **fixing_causes との整合**: `ci_fix_count` は「Fixing に遷移した回数」であり `fixing_causes`（何が問題か）と意味が異なる → 明確にドキュメント化

## References
- Issue #147 設計仕様: `.cupola/inputs/issue.md`
- Issue #126 (Supersedes): Fixing ↔ ReviewWaiting 間の無限ループ防止機構
