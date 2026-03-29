# Implementation Plan

## Requirements Coverage

| Requirement | Tasks |
|-------------|-------|
| 1.1, 1.2, 1.3, 1.4, 1.5 | 1.1, 1.2, 4.1, 5.1, 5.2 |
| 2.1, 2.2, 2.3, 2.4, 2.5 | 3.1, 4.2, 5.1, 5.2, 6.1 |
| 3.1, 3.2, 3.3 | 3.2, 5.1, 5.2 |
| 4.1, 4.2, 4.3, 4.4, 4.5 | 3.2, 4.3, 5.1, 5.2, 6.1 |
| 5.1, 5.2, 5.3, 5.4, 5.5 | 3.1, 4.2, 5.1, 5.2, 6.1 |
| 6.1, 6.2, 6.3, 6.4 | 3.2, 5.1, 5.2, 5.3 |
| 7.1, 7.2, 7.3, 7.4, 7.5, 7.6 | 4.4, 5.1, 5.2, 5.3 |
| 8.1, 8.2, 8.3 | 4.2, 5.1 |
| 9.1, 9.2, 9.3, 9.4, 9.5, 9.6 | 2.1, 2.2, 7.1 |
| 10.1, 10.2, 10.3, 10.4 | 4.2, 5.1, 5.4, 7.1 |
| 11.1, 11.2, 11.3, 11.4 | 3.1, 3.2, 4.2, 4.3, 6.1 |
| 12.1, 12.2, 12.3, 12.4 | 2.2 |

## Tasks

- [x] 1. ドメイン層の構築
- [x] 1.1 ステートマシンの実装
  - 10 状態の State enum を定義し、ヘルパーメソッド（needs_process, is_terminal, is_review_waiting）を実装する
  - Event enum を定���し、GitHub 起因イベント（IssueDetected, IssueClosed, DesignPrMerged, ImplementationPrMerged, UnresolvedThreadsDetected）と内部起因イベント（InitializationCompleted, ProcessCompletedWithPr, ProcessCompleted, ProcessFailed, RetryExhausted）を列挙する
  - StateMachine::transition 関数を純粋関数として実装し、全ての有効な (State, Event) 組み合わせの遷移を網羅する。無効な遷移には TransitionError を返す
  - 全 (State, Event) 組み合わせのユニットテストを作成し、正常遷移と不正遷移の両方を検証する
  - _Requirements: 1.3, 2.3, 3.2, 4.3, 5.3, 6.2, 6.3, 7.1, 7.2_

- [x] 1.2 (P) エンティティと値オブジェクトの定義
  - Issue エンティティを定義し、状態管理に必要なフィールド（github_issue_number, state, design_pr_number, impl_pr_number, worktree_path, retry_count, current_pid, error_message, created_at, updated_at）を持たせる
  - Config 値オブジェクトを定義し、owner, repo, default_branch, language, polling_interval_secs, max_retries, stall_timeout_secs, log_level, log_dir のフィールドとデフォルト��生成メソッドを実装する
  - ExecutionLog 構造体を定義し、issue_id, state, started_at, finished_at, exit_code, structured_output, error_message のフィールドを持たせる
  - _Requirements: 9.1_

- [x] 2. 設定管理とプロジェクト基盤
- [x] 2.1 設定ファイルのパースと CLI 定義
  - cupola.toml を読み込む CupolaToml 構造体を定義し、toml クレートでデシリアライズする。owner, repo, default_branch を必須、その他をオプション（デフォルト値付き）とする
  - CupolaToml から Config への変換ロジックを実装し、CLI フラグによるオーバーライド���適用する（優先順位: CLI > cupola.toml > デフォルト値）
  - clap で cupola コマンド体系（run, init, status, help）を定義する。run サブコマンドには --polling-interval-secs, --log-level, --config オプションを設ける
  - 設定ファイルが存在しない・パース失敗時のエラーハンドリングを実装し、わかりやすいエラーメッセージで起動を中断する
  - 正常パース・必須項目欠損・CLI オーバーライドのユニットテストを作成する
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

- [x] 2.2 (P) ログ基盤の構築
  - tracing + tracing-subscriber を初期化し、log_level 設定に基づくフィルタリングを実装する
  - log.dir が指定された場合は tracing-appender で日付別ファイル出力（cupola-YYYY-MM-DD.log）を追加し、stderr との二重出力を構成する
  - log.dir 未指定時は stderr のみに出力する
  - issue_number, state をスパンフィールドとして含む構造化ログのパターンを確立する
  - _Requirements: 12.1, 12.2, 12.3, 12.4_

- [x] 3. アダプター層の実装（外部接続）
- [x] 3.1 (P) GitHub REST API クライアントの実装
  - octocrab を使用して GitHubClient trait の REST API メソッドを実装する（list_ready_issues, get_issue, is_issue_open, find_pr_by_branches, is_pr_merged, create_pr, comment_on_issue, close_issue）
  - 認証は gh auth token の出力または GITHUB_TOKEN 環境変数から取得する
  - find_pr_by_branches による冪等性チェック（PR 作成前に同一ブランチ間の既存 PR を検索）を実装する
  - GitHub API のエラーレスポンスを CupolaError::GitHub にマッピングする
  - _Requirements: 1.1, 2.1, 2.2, 5.2, 7.1, 7.5, 7.6, 10.3, 11.1_

- [x] 3.2 (P) GitHub GraphQL API クライアントの実装
  - reqwest を��用して GraphQL エンドポ��ントに POST し、review thread の取得・返信・resolve を実装する
  - unresolved review thread の取得では isResolved == false でフィルタし、各 thread の全コメント（会話の文脈）を含めて返す
  - addPullRequestReviewThreadReply mutation で既存 thread に返信する機能を実装する
  - resolveReviewThread mutation で thread を resolve する機能を実装する
  - reviewThreads と comments の両方で cursor-based pagination（first: 100 + pageInfo）を実装し、100 件超のデータに対応する
  - レスポンスは serde_json::Value でパースし、ReviewThread / ReviewComment 構造体にマッピングする
  - _Requirements: 3.1, 3.3, 4.1, 4.2, 6.1, 6.3, 11.1_

- [x] 3.3 (P) GitHubClient facade の合成
  - GitHubClientImpl 構造体を定義し、REST クライアントと GraphQL クライア���トを内部に保持する
  - GitHubClient trait を実装し、各メソッドを適切な内部クライアントに委譲する（REST メソッド: list_ready_issues 等、GraphQL メソッド: list_unresolved_threads 等）
  - owner, repo, 認証トークンを両クライアントで共有する構成とする
  - 3.1 と 3.2 の完了後に合成する（3.1, 3.2 は並列実行可能だが、3.3 はそれらに依存）
  - _Requirements: 11.1_

- [x] 3.4 (P) SQLite リポジトリの実装
  - Arc<Mutex<Connection>> で接続を管理し、WAL モード + busy_timeout + foreign_keys の初期化プラグマを適用する
  - issues テーブルと execution_log テーブルの CREATE TABLE IF NOT EXISTS によるスキーマ初期化を実装する
  - IssueRepository trait の全メソッドを実装する（find_by_id, find_by_issue_number, find_active, find_needing_process, save, update_state, update, reset_for_restart）
  - ExecutionLogRepository trait の全メソッドを実装��る（record_start, record_finish, find_by_issue）
  - 全 DB 操作を tokio::task::spawn_blocking でラップし、非同期コンテキストからの安全なアクセスを保証する
  - reset_for_restart では state を idle にリセットし、PR 番号・worktree_path・retry_count・current_pid・error_message を NULL/0 にクリアする
  - in-memory SQLite を使用した CRUD のユニットテストを作成する
  - _Requirements: 1.1, 1.5, 10.1_

- [x] 3.5 (P) Claude Code プロセス起動の実装
  - std::process::Command で Claude Code を子プロセスとして起動する ClaudeCodeProcess を実装する
  - 起動フラグとして -p（プロンプト）、--output-format json、--dangerously-skip-permissions を常に付与する
  - json_schema が指定された場合は --json-schema フラグを追加する
  - stdout と stderr を Stdio::piped() で取得可能な状態にする
  - _Requirements: 2.2, 5.1, 11.3, 11.4_

- [x] 3.6 (P) Git worktree マネージャーの実装
  - std::process::Command で git コマンドを実行する GitWorktreeManager を実装する
  - worktree の作成（git worktree add -b）、削除（git worktree remove --force）を��等に実装する（対象が存在しない場合はスキップ）
  - ブランチの作成（git checkout -b）、チェックアウト（git checkout）、pull（git pull）、push（git push -u origin）を実装する
  - ブランチ削除はローカル（git branch -D）とリモート（git push origin --delete）の両方を実行し、存在しない場合はスキップする
  - 各コマンドのエラーを CupolaError::Git にマッピングする
  - _Requirements: 1.2, 3.2, 6.4, 7.3, 7.4_

- [x] 4. アプリケーション層のユースケース実装
- [x] 4.1 リトライポリシーとプロンプト構築
  - RetryPolicy を実装し、current_retry_count と max_retries を比較して Retry / Exhausted を判定する
  - build_session_config 関数を実装し、状態に応じたプロンプト文字列と OutputSchemaKind（PrCreation / Fixing）を返す
  - design_running, implementation_running, fixing（設計/実装共通）の 3 種類のプロンプトテンプレートを Rust 文字列定数として定義し、format! で issue_number, language を埋め込む
  - PR 作成用と fixing 用の 2 種類の output-schema JSON 文字列を定数として定義する
  - フォールバック PR タイトル/ボディの生成関数（design: "Design: Issue #N", implementation: "Implement: Issue #N"）を実装する
  - _Requirements: 1.4, 2.4, 4.5, 5.4, 8.3_

- [x] 4.2 SessionManager の実装
  - HashMap<i64, SessionEntry> でプロセスを管理する SessionManager を実装する
  - register メソッドで Child を受け取り、stdout/stderr の読み取りスレッド（std::thread::spawn + BufReader::read_to_string）を起動して JoinHandle を保持する
  - collect_exited メソッドで try_wait() により終了したプロセスを回収し、JoinHandle から stdout/stderr を取得して ExitedSession として返す
  - is_running, kill（単体）, kill_all（全体）メソッドを実装する
  - find_stalled メソッドで起動時刻から timeout を超過したプロセスの issue_id リストを返す
  - _Requirements: 8.1, 8.2, 10.2, 10.4_

- [x] 4.3 入力ファイル書き出しと output-schema パース
  - design_running 時の入力準備として、GitHubIssueDetail を .cupola/inputs/issue.md フォーマット（# Issue #N: title / ## Labels / ## Body）に変換して worktree に書き出す関数を実装する
  - fixing 時の入力準備として、Vec<ReviewThread> を .cupola/inputs/review_threads.json（thread_id, path, line, comments の配列）に変換して worktree に書き出す関数を実装する
  - Claude Code の stdout JSON から PR 作成用の structured_output（pr_title, pr_body）をパースする関数を��装する。パース失敗時は��ォールバック値を使用する
  - Claude Code の stdout JSON から fixing 用の structured_output（threads 配列の thread_id, response, resolved）をパースする関数を実装する。パース失敗時は後処理をスキップする
  - パース成功・失敗・フォールバックのユニットテストを作成する
  - _Requirements: 2.1, 4.1, 4.4, 11.2, 11.3_

- [x] 4.4 TransitionUseCase の実装
  - イベントを受け取り、StateMachine::transition で次状態を決定し、SQLite を更新し、遷移に伴う副作用を実行する TransitionUseCase を実装する
  - Idle → Initialized: SQLite にレコード作成。terminal レコードが存在する場合は reset_for_restart で上書きリセット
  - Initialized → DesignRunning: worktree 作成、issue-main ブランチ作成・push、design ブランチ作成・checkout・push、Issue コメント投稿、retry_count リセット
  - DesignReviewWaiting → ImplementationRunning: worktree を issue-main にチェックアウト、git pull、Issue コメント投稿、retry_count リセット
  - 全正常遷移で retry_count = 0 にリセットする
  - Completed 遷移: SQLite を先に更新 → Issue コメント → Issue close → worktree 削除 → ブランチ削除��design + issue-main）
  - Cancelled 遷移: SQLite を先に更新 → プロセス停止 → Issue close（open なら）→ worktree 削除 → ブランチ削除 → Issue コメント（遷移元に応じて内容分岐）
  - ProcessFailed: retry_count インクリメント、execution_log 記録
  - 同一 Issue に複数イベントがある場合、IssueClosed を最優先で適用する
  - _Requirements: 1.2, 1.3, 1.5, 3.2, 6.4, 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

- [x] 5. PollingUseCase の実装（メインループ）
- [x] 5.1 polling サイクルの 7 ステップ実装
  - tokio::select! で polling interval timer と SIGINT shutdown signal を待機するメインループを構築する
  - Step 1（Issue polling）: list_ready_issues で新規 Issue を検知し IssueDetected イベントを生成。非 terminal Issue に対して個別に is_issue_open で close を確認し IssueClosed イベントを生成
  - Step 2（Initialized 復旧）: Initialized 状態の Issue に対して初期化処理を再試行し、成功時は即時 DesignRunning に遷移。失敗時は retry_count++ し上限到達で Cancelled
  - Step 3（プロセス終了確認）: collect_exited で終了プロセスを回収。running 状態の正常終了では output-schema をパースし PR を作成、fixing 状態では返信 + resolve を実行。異常終了では RetryPolicy で判定し ProcessFailed または RetryExhausted イベントを生成
  - Step 4（PR 監視）: review_waiting 状態の Issue に対し、merge を優先確認（PrMerged）、未 merge なら unresolved thread を確認（UnresolvedThreadsDetected）
  - Step 5（stall 検知）: find_stalled で timeout 超過プロセスを検出し kill → ProcessFailed として処理
  - Step 6（イベント適用）: 収集した全イベントを TransitionUseCase に渡す。同一 Issue の IssueClosed を最優先
  - Step 7（プロセス起動）: needs_process かつ SessionManager にプロセスがない Issue を検出。index.lock 残存を削除。入力ファイル書き出し → Claude Code を spawn → SessionManager に登録
  - _Requirements: 1.1, 1.4, 2.1, 2.2, 2.4, 2.5, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4, 5.5, 6.1, 6.2, 6.3, 7.1, 7.2, 8.1, 8.2, 8.3, 10.2, 11.2, 11.3_

- [x] 5.2 後処理の実装（PR 作成 / コメント返信 + resolve）
  - running 状態の正常終了後の PR 作成ロジックを実装する。冪等性チェック（DB の既存 PR 番号 → find_pr_by_branches で GitHub 上の既存 PR）を経て、未存在なら create_pr を実行。PR 番号を DB に記録
  - fixing 状態の正常終了後のコメント返�� + resolve ロジックを実装する。各 thread を個別に処理し、reply_to_thread → resolve_thread（resolved == true の場合）を実行。個別エラーはログ記録のみで続行
  - output-schema パース失敗時���分岐を実装する。running 状態ではフォールバック値で PR 作成を試行、fixing 状態では後処理をスキップし review_waiting に遷移
  - execution_log に structured_output を記録する
  - _Requirements: 2.2, 4.2, 4.4, 5.2, 10.3, 11.1_

- [x] 5.3 (P) Cleanup ロジックの実装
  - completed / cancelled 遷移時の cleanup を実装する。各操作（worktree 削除、ブランチ削除、Issue close、プロセス停止）を個別に try し、対象が存在しない場合はスキップ
  - cleanup の途中で一部操作が失敗しても残りの操作は続行する。エラーはログ記録のみで状態遷移をブロックしない
  - cancelled の Issue コメントを遷移元に応じて分岐する（Issue close 由来: cleanup 実行した旨、RetryExhausted 由来: 失敗理由 + リトライ回数 + cleanup 実行した旨）
  - _Requirements: 6.4, 7.3, 7.4, 7.5, 7.6_

- [x] 5.4 (P) Graceful shutdown と再起動復旧
  - SIGINT 受信時に polling ループを停止し、全プロセスに SIGTERM を送信。10 秒待機後に生存プロセスに SIGKILL を送信する段階的シャットダウンを実装する
  - 起動時に SQLite から全 Issue レコードを読み込み、非 terminal 状態の Issue の current_pid を NULL にクリアする
  - SessionManager を空の状態で開始し、needs_process な Issue は次の polling サイク���でプロセスが再起動されることを確認する
  - _Requirements: 10.1, 10.2, 10.4_

- [x] 6. エラーハンドリングの統合
- [x] 6.1 エラー型の定義と伝搬の統合
  - CupolaError enum を thiserror で定義し、Domain, Database, GitHub, ClaudeCode, Git, Config のバリアントを設ける
  - 各アダプターのエラーを適切なバリアントにマッピングする（rusqlite::Error → Database、octocrab エラー → GitHub 等）
  - PollingUseCase 内のエラーハンドリング方針を統合する: GitHub API / SQLite エラーはログ記録 + 次サイクル再試行、Claude Code 異常終了は ProcessFailed イベント、cleanup エラーは非ブロッキング
  - _Requirements: 2.4, 4.4, 4.5, 5.4, 11.1_

- [x] 7. Bootstrap と結合
- [x] 7.1 DI 配線とアプリケーション起動
  - main.rs から bootstrap/app.rs を呼び出し、cupola.toml のパース → Config 構築 → CLI フラグオーバ���ライド → tracing 初期化の起動シーケンスを実装する
  - 各アダプターの具象型（GitHubClientImpl, SqliteIssueRepository, SqliteExecutionLogRepository, ClaudeCodeProcess, GitWorktreeManager）を生成し、PollingUseCase に注入する
  - cupola run: PollingUseCase を起動して polling ループを開始する
  - cupola init: SQLite スキーマの冪等な初期化を実行する
  - cupola status: find_active で全 Issue の状態を一覧表示する（#42 design_review_waiting PR:#85 retry:0 の形式）
  - Cargo.toml に全クレート依存（tokio, clap, octocrab, reqwest, rusqlite, serde, serde_json, toml, tracing, tracing-subscriber, tracing-appender, thiserror, anyhow, chrono）を定義する
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 10.4_

- [x] 8. 統合テスト
- [x] 8.1 ステートマシン統合テスト
  - StateMachine + TransitionUseCase の統合テストを作成する。モック adapter（in-memory SQLite + スタブ GitHub / Git）を注入し、主要な遷移パス（idle → completed、idle → cancelled）をエンドツーエンドで検証する
  - retry_count のリセットタイミング（正常遷移時に 0 にリセットされること）を検証する
  - IssueClosed イベントの優先適用（同一 Issue に複数イベントがある場合）を検証する
  - _Requirements: 1.3, 2.3, 2.4, 7.1, 7.2_

- [x] 8.2 (P) Polling サイクル統合テスト
  - PollingUseCase の 1 サイクルをモック adapter で実行し、7 ステップの処理フローを検証する
  - Issue 検知 → Initialized → DesignRunning → プロセ���起動の一連のフローを確認する
  - PR merge 検知 → ImplementationRunning 遷移のフローを確認する
  - stall 検知 → kill → ProcessFailed → リトライのフローを確認する
  - _Requirements: 1.1, 1.4, 3.1, 3.2, 8.1, 8.2_

- [x] 8.3 (P) SessionManager 統合テスト
  - 実際のプロセス（sleep コマンド等）を使用して register → collect_exited → stdout 読み取りのライフサイクルを検証する
  - kill による強制停止と find_stalled によるタイムアウト検知を検証する
  - _Requirements: 8.1, 10.2, 10.4_
