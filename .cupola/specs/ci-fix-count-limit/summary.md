# ci-fix-count-limit

## Feature
Fixing ↔ ReviewWaiting 間で発生しうる無限ループを防ぐため、CI 失敗（CiFailure）または Conflict を起因とする修正試行回数 `ci_fix_count` に上限を設ける。上限到達時は Issue をキャンセルせず ReviewWaiting のまま維持し、Issue コメントで手動介入を促す。Issue #126 の後継機構。

## 要件サマリ
- `Issue.ci_fix_count: u32`（default 0）を追加し、SQLite に永続化。
- `Config.max_ci_fix_cycles: u32`（default 3）を追加、TOML でオーバーライド可能。0 は起動時エラー。
- `step4_pr_monitoring` の causes 評価後のカウンタ操作:
  - causes 空 → 0 にリセット、`FixingRequired` emit せず
  - ReviewComments を含む（CiFailure 同時含む場合も）→ 0 にリセット、emit あり
  - CiFailure/Conflict のみ かつ count < max → +1、emit あり
  - CiFailure/Conflict のみ かつ count >= max → 無変更、emit せず、到達瞬間のみ 1 回 Issue コメント投稿
- `DesignPrMerged` 時にフェーズ変更として `ci_fix_count = 0` にリセット。
- 上限到達時も Cancelled に遷移させず、PR は open のまま保持。人間の修正プッシュで CI 通過後は次サイクルから通常フロー再開。
- SQLite は `ALTER TABLE issues ADD COLUMN ci_fix_count INTEGER NOT NULL DEFAULT 0` でマイグレーション（冪等）。

## アーキテクチャ決定
- **カウンタ管理の配置**: 3 案検討。(A) step4_pr_monitoring 内で一括管理、(B) StateMachine に組み込む、(C) 別 UseCase 分離。(A) 採用。理由: emit 直前の判断が自然で既存ロジックに最小加算、(B) は domain に application ロジックが混入し Clean Architecture 違反、(C) はこの規模には過設計。
- **リセット条件 (CiFailure + ReviewComments 同時)**: ReviewComments を優先してリセット。理由: review 修正でコードが書き換われば CI 状況も変わるため、既存カウントは意味を失う。Issue #147 の設計方針と一致。ReviewComments 修正で CI を通さぬまま上限がリセットされ続ける懸念はあるが、人間レビュアー介在を重視して許容。
- **上限到達通知タイミング**: 毎サイクルコメントではなく `ci_fix_count == max_ci_fix_cycles` のちょうど到達したサイクルのみ 1 回投稿。Issue コメントスパムを回避。再起動後の重複投稿リスクは許容範囲。
- **フェーズ変更時のリセット位置**: step4 ではなく `transition_use_case.execute_side_effects` の `DesignPrMerged` ブランチに配置。フェーズ変更副作用の集約場所として既存パターンに合致。
- **`retry_count` との分離**: `retry_count` は ProcessFailed 起因のリトライ用途で意味が異なるため、別フィールドとして新設し混同を避ける。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `Issue.ci_fix_count` | domain | CI/Conflict 起因修正試行回数の保持 |
| `Config.max_ci_fix_cycles` | domain | 上限値の設定 |
| `step4_pr_monitoring`（拡張） | application | causes に応じたカウンタ操作・上限チェック・通知 |
| `transition_use_case.execute_side_effects`（拡張） | application | `DesignPrMerged` 時のリセット |
| `SqliteIssueRepository`（拡張） | adapter/outbound | `ci_fix_count` の読み書き |
| `SqliteConnection`（マイグレーション） | adapter/outbound | `ALTER TABLE` 実行 |
| `config_loader`（拡張） | bootstrap | `max_ci_fix_cycles` TOML パースとバリデーション |

## 主要インターフェース
- `Issue { ci_fix_count: u32, .. }`（default 0、`retry_count` と同パターンで追加）
- `Config { max_ci_fix_cycles: u32, .. }`（default 3、`max_retries` と同パターン）
- `ALTER TABLE issues ADD COLUMN ci_fix_count INTEGER NOT NULL DEFAULT 0`（既存 `IF NOT EXISTS` 相当のマイグレーションブロックに追加）
- step4 判定ヘルパ: `contains_ci_or_conflict(causes)` / `contains_review_comments(causes)`
- Issue コメント文言: 「CI/Conflict の修正が上限に達しました。手動確認してください。」

## 学び / トレードオフ
- 上限到達「瞬間」を `count == max` で検出する方式は、count が変化しなくなるためスパム防止に有効だが、プロセス再起動で再度 `count == max` を経由すればコメント重複する可能性がある。許容範囲と判断。
- `ci_fix_count` と `fixing_causes` の意味を明確に分離（前者は試行回数、後者はその試行の原因リスト）。ドキュメントで区別を強調する必要あり。
- Design → Implementation のフェーズ変更リセットは `DesignPrMerged` の副作用位置に依存しており、将来フェーズが増えた場合は個別追加が必要。
- ReviewComments を含むとリセットする方針は CI が通らない状態でのループを潜在的に許容する。人間介入を最終セーフティネットと位置付ける設計判断。
- Slack/メール等の外部通知、UI 表示、ReviewComments 起因ループ上限はスコープ外。
