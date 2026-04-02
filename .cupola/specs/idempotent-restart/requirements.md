# Requirements Document

## Project Description (Input)
Cancelled からの冪等リスタートと cupola cleanup コマンドの追加 - Cancelled 状態からの Issue reopen 時に完了済みフェーズをスキップして効率的に再開できるようにし、不要な worktree を手動削除する cupola cleanup コマンドを追加する

## Introduction

本機能は、Cupola の自動開発エージェントにおいて、Cancelled 状態から Issue を reopen した際の再開フローを最適化する。現状は全フィールドをリセットして Idle から完全にやり直すが、これによりセッションコストの無駄・以前のレビューフィードバック消失・スペック重複作成などの問題が生じている。本要件は、ステートマシンを変更せずに各工程を冪等化し、完了済みのリソース（worktree、PR）があればスキップして再開できるようにする。また、Cancelled 状態に紐づく worktree やブランチを手動掃除する `cupola cleanup` コマンドを追加する。

## Requirements

### Requirement 1: reset_for_restart の部分リセット

**Objective:** 自動化エージェントのオペレーターとして、Cancelled 状態からの reopen 時にリソース情報（worktree・PR 番号・feature_name）を保持したまま状態のみリセットしたい。それにより、以前の作業成果を再利用できセッションコストを削減できる。

#### Acceptance Criteria
1. When Cupola が Issue を `reset_for_restart` する, the Cupola shall `state='idle'`, `retry_count=0`, `current_pid=NULL`, `error_message=NULL` のみリセットし、`design_pr_number`、`impl_pr_number`、`worktree_path`、`feature_name` は保持する
2. The Cupola shall `reset_for_restart` 後も DB に保存されていた `design_pr_number`、`impl_pr_number`、`worktree_path`、`feature_name` の値が変わっていないことを保証する

### Requirement 2: RetryExhausted 時の worktree 保持

**Objective:** 自動化エージェントのオペレーターとして、Cancelled + RetryExhausted 遷移時に worktree とブランチを削除せずに保持したい。それにより、次回の reopen 時に既存リソースを再利用できる。

#### Acceptance Criteria
1. When `(State::Cancelled, Event::RetryExhausted)` の遷移が発生する, the Cupola shall worktree の削除を行わず、Issue の GitHub close のみを実行する
2. When `(State::Cancelled, Event::IssueClosed)` の遷移が発生する（人間が意図的に close した場合）, the Cupola shall cleanup（worktree 削除・ブランチ削除）を実行する
3. The Cupola shall RetryExhausted による Cancelled 遷移の後も、対象の worktree パスとブランチ（`cupola/{N}/main`、`cupola/{N}/design`）がファイルシステムおよびリポジトリに存在し続けることを保証する

### Requirement 3: initialize_issue の冪等化

**Objective:** 自動化エージェントのオペレーターとして、worktree が既に存在する場合は再作成をスキップして既存 worktree を再利用したい。それにより、Cancelled 後の reopen 時にエラーなく処理が継続できる。

#### Acceptance Criteria
1. When Cupola が `initialize_issue` を実行し、指定パスの worktree が既に存在する, the Cupola shall 新規作成をスキップして既存 worktree をそのまま使用する
2. When Cupola が `initialize_issue` を実行し、指定パスの worktree が存在しない, the Cupola shall 通常通り新規 worktree を作成する
3. When 既存 worktree を再利用する場合, the Cupola shall GitHub Issue に「再開します」系のメッセージ（i18n 対応）をコメントとして投稿する
4. When 新規 worktree を作成する場合, the Cupola shall GitHub Issue に初回開始のメッセージをコメントとして投稿する

### Requirement 4: DesignRunning / ImplementationRunning でのスポーン前 PR チェック

**Objective:** 自動化エージェントのオペレーターとして、プロセスをスポーンする前に DB の PR 番号を確認し、マージ済み・オープンの PR があれば対応するイベントを発火したい。それにより、完了済みフェーズのセッション再実行を防ぎコストを削減できる。

#### Acceptance Criteria
1. When Cupola が DesignRunning 状態でプロセスをスポーンしようとし、DB の `design_pr_number` に値があり、その PR がマージ済みである, the Cupola shall `DesignPrMerged` イベントを発火してスポーンをスキップする
2. When Cupola が DesignRunning 状態でプロセスをスポーンしようとし、DB の `design_pr_number` に値があり、その PR がオープンである, the Cupola shall `ProcessCompletedWithPr` イベントを発火して ReviewWaiting へ遷移させる
3. When Cupola が ImplementationRunning 状態でプロセスをスポーンしようとし、DB の `impl_pr_number` に値があり、その PR がマージ済みである, the Cupola shall `ImplementationPrMerged` イベントを発火してスポーンをスキップする
4. When Cupola が ImplementationRunning 状態でプロセスをスポーンしようとし、DB の `impl_pr_number` に値があり、その PR がオープンである, the Cupola shall `ProcessCompletedWithPr` イベントを発火して ReviewWaiting へ遷移させる
5. When DB の PR 番号が存在しない、または対応する PR が close 済み（not merged）である, the Cupola shall 通常通りセッションを起動する
6. If PR の状態確認中に GitHub API エラーが発生する, the Cupola shall エラーをログに記録し、通常通りセッションを起動する（フェイルセーフ）

### Requirement 5: cupola cleanup コマンド

**Objective:** 自動化エージェントのオペレーターとして、Cancelled 状態の Issue に紐づく worktree・ブランチを手動で一括削除したい。それにより、ディスクスペースや Git リポジトリを整理できる。

#### Acceptance Criteria
1. When オペレーターが `cupola cleanup` を実行する, the Cupola shall DB から Cancelled 状態の全 Issue を取得する
2. When Cancelled 状態の Issue の `worktree_path` が存在する, the Cupola shall 対象ディレクトリを削除する
3. When Cancelled 状態の Issue のブランチ（`cupola/{N}/main`、`cupola/{N}/design`）が存在する, the Cupola shall 対象ブランチを削除する
4. When cleanup が完了した, the Cupola shall DB の `worktree_path`、`design_pr_number`、`impl_pr_number`、`feature_name` を NULL にリセットする
5. The Cupola shall `cupola cleanup` の実行結果（削除した worktree・ブランチの一覧）を標準出力に表示する
6. If 削除対象の worktree やブランチが既に存在しない, the Cupola shall エラーを発生させずにスキップして処理を続行する

### Requirement 6: i18n 対応（再開メッセージ）

**Objective:** 自動化エージェントのオペレーターとして、reopen 時の GitHub Issue コメントが日英両対応になっていることを確認したい。それにより、利用言語に応じた適切なメッセージが投稿される。

#### Acceptance Criteria
1. The Cupola shall `locales/en.yml` に `issue_comment.resuming_design: "Resuming design"` を含む
2. The Cupola shall `locales/ja.yml` に `issue_comment.resuming_design: "設計を再開します"` を含む
3. The Cupola shall `locales/en.yml` に `issue_comment.resuming_implementation: "Resuming implementation"` を含む
4. The Cupola shall `locales/ja.yml` に `issue_comment.resuming_implementation: "実装を再開します"` を含む
5. When 既存 worktree を再利用して設計フェーズを再開する, the Cupola shall 設定言語に対応した resuming_design メッセージを GitHub Issue にコメントする
6. When 既存 worktree を再利用して実装フェーズを再開する, the Cupola shall 設定言語に対応した resuming_implementation メッセージを GitHub Issue にコメントする
