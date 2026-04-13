# Implementation Plan

- [ ] 1. CleanupWorktree エフェクトにブランチ削除を追加する
- [ ] 1.1 ポーリングエグゼキュータの CleanupWorktree ブロックにブランチ削除ロジックを追加する
  - `src/application/polling/execute.rs` の `Effect::CleanupWorktree` ブロックを修正する
  - ワークツリー削除後に `format!("cupola/{}/main", issue.feature_name)` と `format!("cupola/{}/design", issue.feature_name)` のブランチ名を生成する
  - 各ブランチに対して `worktree.delete_branch(branch)` をベストエフォートで呼び出す
  - 成功時は `tracing::info!`、失敗時は `tracing::warn!` でログを記録する
  - `worktree_path` のメタデータクリアはブランチ削除の結果に関わらず継続する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 2. CleanupWorktree のブランチ削除動作をユニットテストで検証する
- [ ] 2.1 (P) 正常系テスト: 両ブランチへの delete_branch 呼び出しを検証する
  - `MockGitWorktree` に `delete_branch` の呼び出し記録機能を追加する（既存モックを活用）
  - `CleanupWorktree` エフェクト実行後に `cupola/{feature_name}/main` と `cupola/{feature_name}/design` の両方が削除されたことを確認する
  - _Requirements: 3.1_

- [ ] 2.2 (P) エラー系テスト: delete_branch の失敗がエフェクト全体に影響しないことを検証する
  - `delete_branch` が `Err` を返すモックを用意する
  - `CleanupWorktree` エフェクトが `Ok(())` を返すことを確認する
  - `worktree_path` のメタデータクリアが実行されることを確認する
  - _Requirements: 3.2, 1.4_

- [ ] 2.3 (P) 境界値テスト: worktree_path が None の場合にブランチ削除をスキップすることを検証する
  - `issue.worktree_path` が `None` の状態で `CleanupWorktree` エフェクトを実行する
  - `delete_branch` が呼ばれないことを確認する
  - _Requirements: 1.5_

- [ ] 3. Cancelled 状態のリグレッションテストを確認する
- [ ] 3.1 Cancelled イシューに対してブランチ削除が実行されないことをリグレッションテストで確認する
  - 既存の状態機械テストで `Cancelled` 状態の Issue に `Effect::CleanupWorktree` が生成されないことを確認する
  - 必要に応じてテストケースを追加・補強する
  - _Requirements: 2.1, 2.2, 3.3_
