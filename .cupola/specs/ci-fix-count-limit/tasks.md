# 実装タスク一覧

## タスク概要

| # | タスク | 要件カバレッジ |
|---|--------|--------------|
| 1 | ドメイン層への新規フィールド追加 | 1.1, 2.1 |
| 2 | SQLite スキーマとリポジトリの更新 | 5.1, 5.2, 5.3, 5.4 |
| 3 | 設定ローダーへの max_ci_fix_cycles 追加 | 2.2, 2.3 |
| 4 | step4_pr_monitoring のカウンタ管理ロジック実装 | 1.2, 1.3, 1.4, 1.5, 1.6, 3.1, 3.2, 3.3, 3.4 |
| 5 | フェーズ変更時の ci_fix_count リセット実装 | 4.1 |
| 6 | テストの実装 | 全要件 |

---

- [x] 1. ドメイン層への新規フィールド追加

- [x] 1.1 Issue エンティティに ci_fix_count フィールドを追加する
  - `Issue` 構造体に `ci_fix_count: u32` フィールドを追加し、デフォルト値を 0 にする
  - `retry_count: u32` の定義箇所に倣って同様のパターンで追加する
  - `Issue` の新規作成時に `ci_fix_count` が 0 で初期化されることを確認する
  - _Requirements: 1.1_

- [x] 1.2 Config に max_ci_fix_cycles フィールドを追加する
  - `Config` 構造体に `max_ci_fix_cycles: u32` フィールドを追加し、デフォルト値を 3 にする
  - `max_retries: u32` の定義パターンに倣って実装する
  - _Requirements: 2.1_

- [x] 2. SQLite スキーマとリポジトリの更新

  （タスク 1 完了後に実施）

- [x] 2.1 (P) マイグレーションで ci_fix_count カラムを追加する
  - 既存のマイグレーションブロックに `ALTER TABLE issues ADD COLUMN ci_fix_count INTEGER NOT NULL DEFAULT 0` を追加する
  - カラムが既に存在する場合はスキップする（既存の `IF NOT EXISTS` 相当のパターンを踏襲）
  - `CREATE TABLE` 文にも `ci_fix_count INTEGER NOT NULL DEFAULT 0` を追加する（将来の init 向け）
  - _Requirements: 5.1, 5.4_

- [x] 2.2 (P) リポジトリの読み書きロジックに ci_fix_count を追加する
  - `row_to_issue()` 関数で `ci_fix_count` カラムを読み取り、`Issue.ci_fix_count` にマッピングする
  - INSERT クエリに `ci_fix_count` カラムと値を追加する
  - UPDATE クエリに `ci_fix_count` の更新を追加する
  - `retry_count` の読み書きパターンに倣って実装する
  - _Requirements: 5.2, 5.3_

- [x] 3. 設定ローダーへの max_ci_fix_cycles 追加

  （タスク 1 完了後に実施。タスク 2 と並行実施可能）

- [x] 3.1 (P) config_loader で max_ci_fix_cycles を TOML からパースする
  - `config_loader.rs` で `max_ci_fix_cycles` を TOML から読み取り `Config` にセットする
  - TOML に未指定の場合はデフォルト値 3 を使用する（`max_retries` の実装パターンを踏襲）
  - `max_ci_fix_cycles = 0` が設定された場合は起動時にエラーを返す
  - _Requirements: 2.2, 2.3_

- [x] 4. step4_pr_monitoring のカウンタ管理ロジック実装

  （タスク 1・2・3 完了後に実施）

- [x] 4.1 causes の内容に基づいてカウンタを操作する分岐ロジックを実装する
  - `causes` が空の場合に `ci_fix_count` を 0 にリセットし、`FixingRequired` を emit しないようにする
  - `causes` に `ReviewComments` が含まれる場合に `ci_fix_count` を 0 にリセットし、`FixingRequired` を emit するようにする（`CiFailure` との同時発生も含む）
  - `causes` に `CiFailure` または `Conflict` のみが含まれ、かつ `ci_fix_count < max_ci_fix_cycles` の場合に `ci_fix_count` を 1 増加させて `FixingRequired` を emit するようにする
  - _Requirements: 1.2, 1.3, 1.4, 1.6_

- [x] 4.2 上限到達時の処理を実装する
  - `causes` に `CiFailure` または `Conflict` が含まれ、かつ `ci_fix_count >= max_ci_fix_cycles` の場合に `FixingRequired` を emit しないようにする
  - `ci_fix_count == max_ci_fix_cycles` の瞬間（ちょうど到達したサイクル）に GitHub Issue へのコメントを 1 回投稿する
  - コメント内容: 「CI/Conflict の修正が上限に達しました。手動確認してください。」
  - Issue の状態（ReviewWaiting）および PR（open）をそのまま維持することを確認する
  - _Requirements: 1.5, 3.1, 3.2, 3.3_

- [x] 4.3 ci_fix_count の変更後に必ず DB へ永続化する
  - カウンタのリセット・インクリメントを行った後に `issue_repo.update()` を呼び出す
  - 人間の手動プッシュで CI が通過した次のポーリングサイクルで通常フローが再開されることを確認する（causes が空 → リセット → ReviewWaiting → 次の評価へ）
  - _Requirements: 3.4, 5.2_

- [x] 5. フェーズ変更時の ci_fix_count リセット実装

  （タスク 1・2 完了後に実施。タスク 4 と並行実施可能）

- [x] 5.1 (P) DesignPrMerged イベント処理時に ci_fix_count をリセットする
  - `transition_use_case.execute_side_effects` の `DesignPrMerged` ブランチで `issue.ci_fix_count = 0` を設定する
  - リセット後に `issue_repo.update()` を呼び出して永続化する
  - _Requirements: 4.1_

- [x] 6. テストの実装

  （タスク 1〜5 完了後に実施）

- [x] 6.1 (P) causes 評価ロジックのユニットテストを実装する
  - causes 空 → リセット・emit なしのケース
  - ReviewComments のみ → リセット・emit ありのケース
  - CiFailure のみ・上限未満 → インクリメント・emit ありのケース
  - CiFailure のみ・上限以上 → カウント変化なし・emit なしのケース
  - CiFailure + ReviewComments 同時 → リセット・emit ありのケース
  - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6_

- [x] 6.2 (P) Config と config_loader のユニットテストを実装する
  - `max_ci_fix_cycles` のデフォルト値が 3 であることを確認する
  - `max_ci_fix_cycles = 0` がエラーを返すことを確認する
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 6.3 SQLite 読み書きのインテグレーションテストを実装する
  - インメモリ DB で `ci_fix_count` の INSERT・UPDATE・SELECT が正常に動作することを確認する
  - マイグレーション適用前のスキーマから `ci_fix_count` カラムが追加されることを確認する
  - _Requirements: 5.2, 5.3, 5.4_

- [x] 6.4 Fixing ↔ ReviewWaiting ループ上限のインテグレーションテストを実装する
  - `max_ci_fix_cycles = 2` でモック CI 失敗を設定し、2 回目の上限到達で ReviewWaiting に留まることを確認する
  - Design フェーズで `ci_fix_count = 2` になった後、`DesignPrMerged` で 0 にリセットされることを確認する
  - _Requirements: 1.4, 1.5, 3.2, 3.3, 4.1_
