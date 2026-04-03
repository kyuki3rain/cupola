# Implementation Plan

- [ ] 1. GitWorktreeトレイトにmergeメソッドを追加する
  - `application/port` の `GitWorktree` トレイトに `merge(worktree_path, branch)` メソッドを追加する
  - 戻り値は `Result<()>`（成功時 `Ok(())`、コンフリクト含む失敗時 `Err`）とする
  - `Send + Sync` 境界を既存メソッドと同様に維持する
  - _Requirements: 5.1_

- [ ] 2. (P) GitWorktreeManagerにmerge()を実装する
- [ ] 2.1 (P) merge()メソッドの実装を追加する
  - `adapter/outbound` の `GitWorktreeManager` に `GitWorktree::merge()` の実装を追加する
  - `run_git_in_dir` を使って `git merge --no-edit origin/{branch}` を実行する
  - `git merge` が非0終了コードを返した場合（コンフリクト含む）は `Err` を返す
  - エラーコンテキストメッセージに対象ブランチ名を含める
  - Task 1 のトレイト変更が前提
  - _Requirements: 2.2, 5.2, 5.3, 5.4_

- [ ] 2.2 (P) GitWorktreeManagerのmerge()ユニットテストを追加する
  - tempdir と bare origin リポジトリを使ってクリーンマージが成功することを検証する
  - 同一ファイルを両ブランチで異なる内容に変更し、コンフリクト発生時に `Err` が返ることを検証する
  - Task 2.1 の実装が前提
  - _Requirements: 5.2, 5.4_

- [ ] 3. (P) build_fixing_promptとbuild_session_configにコンフリクトフラグを追加する
  - Task 1 とは独立して実装可能（prompt.rs のみを変更する）
  - Task 2 と並行して実施できる

- [ ] 3.1 (P) has_merge_conflictフラグをシグネチャに追加してコンフリクト解消指示を挿入する
  - `application/prompt.rs` の `build_fixing_prompt` に `has_merge_conflict: bool` パラメータを追加する
  - `has_merge_conflict == true` の場合、プロンプト本文の冒頭（"Actions Required" セクションより前）にコンフリクト解消セクションを英語で挿入する
  - 挿入内容にはコンフリクトマーカー確認・`git add` によるステージング・`git commit --no-edit` によるマージ完了・その後の修正作業への移行の4ステップを含める
  - `build_session_config` にも `has_merge_conflict: bool` パラメータを追加し、DesignFixing / ImplementationFixing の両パスで `build_fixing_prompt` に転送する
  - _Requirements: 3.3, 3.4, 3.5, 6.1, 6.2, 6.3_

- [ ] 3.2 build_fixing_promptのユニットテストを追加する
  - `has_merge_conflict=true` の場合にプロンプト先頭にコンフリクト解消セクションが含まれることを検証する
  - コンフリクト解消セクションが "Actions Required" より前に出現することを検証する
  - `has_merge_conflict=false` の場合は既存プロンプトと同一内容であることを検証する
  - Task 3.1 の実装が前提
  - _Requirements: 3.3, 3.4, 3.5, 6.2, 6.3_

- [ ] 4. step7_spawn_processesにfetch+mergeロジックを組み込む
  - Task 2 と Task 3 の両方が完了していることが前提

- [ ] 4.1 Fixing状態のspawn前にfetch・mergeを実行するロジックを追加する
  - `step7_spawn_processes` 内のworktree確認直後（index.lockクリーンアップ前）にFixing状態判定を追加する
  - Fixing状態（DesignFixing / ImplementationFixing）の場合のみfetch・mergeを実行する
  - fetch失敗時は警告ログを出力してmergeをスキップし、spawn処理を継続する（spawn全体をブロックしない）
  - merge成功（クリーンマージ）時はそのままspawnに進む（causesへの追加なし）
  - mergeコンフリクト時は `has_merge_conflict = true` を設定し、ローカルの causes リストに `Conflict` を追加してspawnを継続する
  - コンフリクト以外のmerge失敗時は警告ログを出力してspawnを継続する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.3, 3.1, 3.2, 4.1, 4.2, 4.3_

- [ ] 4.2 has_merge_conflictフラグをbuild_session_configに渡す
  - Task 4.1 で設定した `has_merge_conflict` フラグを `build_session_config` 呼び出し時に渡す
  - Fixing状態以外では `false` を渡し、既存の動作を変更しない
  - _Requirements: 6.1_

- [ ] 4.3 step7のfetch+mergeロジックのユニットテストを追加する
  - MockGitWorktree を使い、Fixing状態で `fetch()` と `merge()` が呼ばれることを検証する
  - `fetch()` が失敗した場合にspawnが継続されること（merge は呼ばれない）を検証する
  - `merge()` がコンフリクトエラーを返した場合に `has_merge_conflict=true` でspawnが呼ばれることを検証する
  - `DesignRunning` 等のFixing以外の状態ではfetch・mergeが呼ばれないことを検証する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.3, 3.1, 3.2, 4.1, 4.2, 4.3_
