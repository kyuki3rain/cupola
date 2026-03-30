# Implementation Plan

## Tasks

- [ ] 1. FixingProblemKind ドメイン型と Issue エンティティの拡張
- [ ] 1.1 FixingProblemKind 列挙型を domain 層に追加する
  - CI失敗・conflict・reviewコメントを表す3つのバリアント（CiFailure, Conflict, ReviewComments）を持つ列挙型を実装する
  - serde の `rename_all = "snake_case"` でJSON文字列とのシリアライズを定義する
  - `Debug, Clone, PartialEq, Eq` を derive する
  - `domain/mod.rs` に新規モジュールとして公開する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 1.2 Issue エンティティに fixing_causes フィールドを追加してDBスキーマを移行する
  - `Issue` 構造体に `fixing_causes: Vec<FixingProblemKind>` フィールドを追加する
  - SQLite の `issues` テーブルに `fixing_causes TEXT NOT NULL DEFAULT '[]'` カラムを追加するスキーマ移行を実装する（`ALTER TABLE IF NOT EXISTS` またはスキーマ初期化時の対応）
  - `SqliteIssueRepository` の CRUD 操作（insert/update/find）で `fixing_causes` を JSON 文字列に変換して保存・読み込みする
  - 既存の `Issue` 構築箇所に `fixing_causes: vec![]` のデフォルト値を追加する
  - _Requirements: 3.5, 4.1, 5.1, 5.2_

- [ ] 2. GitHub CI確認・conflict確認 API の追加
- [ ] 2.1 GitHubClient ポートに CI check-runs と PR mergeable 取得メソッドを追加する
  - `GitHubCheckRun` 構造体（id, name, status, conclusion フィールド）を port 定義に追加する
  - `GitHubClient` trait に `get_ci_check_runs(pr_number: u64)` メソッドを追加する（`status == "completed"` の runs のみを返す）
  - `GitHubClient` trait に `get_pr_mergeable(pr_number: u64)` メソッドを追加する（`Option<bool>` を返し、`None` はGitHubが計算中を意味する）
  - _Requirements: 1.1, 1.5, 2.1_

- [ ] 2.2 (P) CI check-runs 取得の REST 実装を OctocrabRestClient に追加する
  - `OctocrabRestClient` に `reqwest::Client` と `token` フィールドを追加して直接 REST 呼び出し対応にする
  - PR の head SHA を `octocrab.pulls().get(pr_number)` で取得する
  - `GET /repos/{owner}/{repo}/commits/{sha}/check-runs` を reqwest で呼び出す
  - `status == "completed"` の runs のみをフィルタリングして返す
  - API 失敗時はエラーを返す（呼び出し元でログとスキップを処理する）
  - _Requirements: 1.1, 1.4, 1.5_

- [ ] 2.3 (P) PR mergeable フィールド取得の REST 実装を OctocrabRestClient に追加する
  - `GET /repos/{owner}/{repo}/pulls/{number}` を reqwest または octocrab で呼び出す
  - レスポンスから `mergeable` フィールドを `Option<bool>` として抽出する
  - `null`（計算中）の場合は `Ok(None)` を返す
  - API 失敗時はエラーを返す
  - _Requirements: 2.1, 2.4_

- [ ] 2.4 GitHubClientImpl ファサードに新規メソッドの委譲を追加する
  - `GitHubClientImpl` の `GitHubClient` impl ブロックに `get_ci_check_runs` と `get_pr_mergeable` を追加する
  - 両メソッドとも `rest` フィールドに委譲する
  - _Requirements: 1.1, 2.1_

- [ ] 3. CI エラー・conflict 情報の入力ファイル書き出し機能を追加する
- [ ] 3.1 (P) CI エラーログを ci_errors.txt に書き出す機能を実装する
  - `CiErrorEntry` 構造体（check_run_name, conclusion, output_summary, output_text フィールド）を `io.rs` に追加する
  - `write_ci_errors_input(worktree_path, errors: &[CiErrorEntry])` 関数を実装する（`.cupola/inputs/ci_errors.txt` に書き出す）
  - 各 check-run のテキストフォーマットは `# CI Failure Report\n## {name}\n{summary}\n{text}` 形式
  - tempdir を使用した単体テストを追加する
  - _Requirements: 1.3_

- [ ] 3.2 (P) conflict 情報を conflict_info.txt に書き出す機能を実装する
  - `ConflictInfo` 構造体（head_branch, base_branch, default_branch フィールド）を `io.rs` に追加する
  - `write_conflict_info_input(worktree_path, info: &ConflictInfo)` 関数を実装する（`.cupola/inputs/conflict_info.txt` に書き出す）
  - ファイル内容は `head_branch: {name}\nbase_branch: {name}\ndefault_branch: {name}` 形式
  - tempdir を使用した単体テストを追加する
  - _Requirements: 2.3, 2.5_

- [ ] 4. fixing プロンプトを問題種別リストに基づく動的生成へ変更する
  - `build_fixing_prompt` の引数に `causes: &[FixingProblemKind]` を追加する
  - `ReviewComments` が含まれる場合: `.cupola/inputs/review_threads.json を参照して修正してください` の指示を追加する
  - `CiFailure` が含まれる場合: `.cupola/inputs/ci_errors.txt を参照して修正してください` の指示を追加する
  - `Conflict` が含まれる場合: `origin/{default_branch} を取り込んでconflictを解消してください` の指示を追加する（`default_branch` は `config` から取得）
  - 複数問題がある場合はすべての指示を1プロンプトに結合する
  - `build_session_config` の引数に `fixing_causes: &[FixingProblemKind]` を追加し、fixing 状態では `issue.fixing_causes` を渡す
  - 既存の `build_fixing_prompt` のテストを更新し、動的生成の各パターンをテストする
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [ ] 5. step4 PR 監視ロジックを拡張して CI 失敗・conflict を検知する
- [ ] 5.1 step4 に CI 失敗検知を追加する
  - マージ確認後（`is_pr_merged` が `false` の場合のみ）に `get_ci_check_runs` を呼び出す
  - `conclusion == "failure"` の runs が存在する場合、`causes` に `CiFailure` を追加し、失敗 runs の情報を一時保持する
  - `get_ci_check_runs` の API 呼び出し失敗時は warn ログを記録してCIチェックをスキップし、他のチェックを継続する
  - _Requirements: 1.1, 1.2, 1.4, 1.5_

- [ ] 5.2 step4 に conflict 検知を追加する
  - CI 確認に続けて（CI 失敗の有無にかかわらず）`get_pr_mergeable` を呼び出す
  - `mergeable == Some(false)` の場合、`causes` に `Conflict` を追加する
  - `mergeable == None`（計算中）の場合はスキップして次サイクルで再確認する
  - `get_pr_mergeable` の API 失敗時は warn ログを記録してスキップし、他のチェックを継続する
  - _Requirements: 2.1, 2.2, 2.4_

- [ ] 5.3 複数問題の同時収集と優先順位制御・イベント発行を実装する
  - CI 確認・conflict 確認に続けて `list_unresolved_threads` を呼び出し、未解決スレッドがあれば `causes` に `ReviewComments` を追加する（既存処理を causes 収集方式に変更）
  - `causes` が空でない場合、`issue.fixing_causes = causes` を設定して `issue_repo.update` で保存し、`UnresolvedThreadsDetected` イベントを発行する
  - `causes` が空の場合はイベントを発行しない（次サイクルへ）
  - `issue_repo.update` 失敗時はエラーをログに記録してイベント発行を中止する
  - 検知完了時に `tracing::info!` で問題数をログに記録する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 5.3_

- [ ] 6. prepare_inputs を fixing_causes に基づく入力ファイル準備方式へ更新する
  - `prepare_inputs` の fixing 状態ブランチで `issue.fixing_causes` を参照して必要なファイルのみを書き出す
  - `ReviewComments` が含まれる場合: `list_unresolved_threads` → `write_review_threads_input`（既存処理を条件分岐化）
  - `CiFailure` が含まれる場合: `get_ci_check_runs` で failure のみ取得 → `write_ci_errors_input`
  - `Conflict` が含まれる場合: ブランチ情報を組み立て → `write_conflict_info_input`
  - ファイル書き出し失敗時は `Err` を返してプロセス起動をスキップする
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 7. 統合テストとユニットテストの追加
- [ ] 7.1 build_fixing_prompt の各パターンのユニットテストを追加する
  - `causes = [ReviewComments]` でプロンプトに `review_threads.json` 参照指示のみが含まれることを検証
  - `causes = [CiFailure]` でプロンプトに `ci_errors.txt` 参照指示のみが含まれることを検証
  - `causes = [Conflict]` でプロンプトに conflict 解消指示のみが含まれることを検証
  - `causes = [CiFailure, Conflict, ReviewComments]` でプロンプトに全指示が含まれることを検証
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [ ] 7.2 step4 の CI 失敗・conflict 検知の統合テストを追加する
  - MockGitHubClient を使用して CI 失敗あり・なしのシナリオをテスト
  - MockGitHubClient を使用して conflict あり・なし（`Some(false)` / `Some(true)` / `None`）のシナリオをテスト
  - CI 失敗 + conflict が同時発生した場合に `fixing_causes` に両方が含まれることを検証
  - CI 失敗 + unresolved thread が同時発生した場合に1回の `UnresolvedThreadsDetected` イベントにまとめられることを検証
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 5.5_

- [ ]* 7.3 prepare_inputs の入力ファイル書き出しを検証するテストを追加する
  - `fixing_causes = [CiFailure]` で `ci_errors.txt` のみ生成されることを tempdir で検証
  - `fixing_causes = [Conflict]` で `conflict_info.txt` のみ生成されることを tempdir で検証
  - `fixing_causes = [ReviewComments, CiFailure]` で両ファイルが生成されることを tempdir で検証
  - _Requirements: 5.1, 5.2, 5.3, 5.4_
