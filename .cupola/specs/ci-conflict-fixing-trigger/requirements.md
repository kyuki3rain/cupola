# Requirements Document

## Introduction

cupola の `review_waiting` 状態において、PRのCI失敗およびconflictを自動検知し、`fixing` 状態へ遷移してClaude Codeに自動修正指示を送る機能を追加する。現状はreviewスレッドの有無とmerge状態のみを監視しており、CI失敗やconflictが発生したPRは人間が手動で対応するか放置される課題がある。本機能により、PRの品質問題を自動検知・修正サイクルに組み込む。

## Requirements

### Requirement 1: CI失敗の検知

**Objective:** As a cupola利用者, I want `review_waiting` 状態でPRのCI失敗を自動検知する, so that CI失敗が発生したPRを人間が気づかなくても自動修正サイクルへ移行できる

#### Acceptance Criteria

1. While `review_waiting` 状態にある, the cupola shall GitHub Checks API（`GET /repos/{owner}/{repo}/commits/{ref}/check-runs`）を使用してPRのCIステータスをpollingで確認する
2. When CIチェックの `conclusion` が `"failure"` である, the cupola shall `fixing` 状態へ遷移するイベントを生成する
3. When CI失敗を検知した, the cupola shall 失敗したcheck-runのログ（`output.text` または `output.summary`）を取得して `.cupola/inputs/ci_errors.txt` としてworktreeに書き出す
4. If GitHub Checks APIの呼び出しに失敗した, the cupola shall エラーをログに記録しそのpollingサイクルをスキップする（状態遷移は行わない）
5. The cupola shall CI失敗検知のためのAPIコールをreview_waiting Issue あたり最大2回/サイクルに収める

### Requirement 2: Conflictの検知

**Objective:** As a cupola利用者, I want `review_waiting` 状態でPRのconflictを自動検知する, so that conflictが発生したPRを自動修正サイクルへ移行できる

#### Acceptance Criteria

1. While `review_waiting` 状態にある, the cupola shall GitHub REST API（`GET /repos/{owner}/{repo}/pulls/{number}`）のPR情報からmergeableフィールドを確認する
2. When `mergeable` フィールドが `false` である, the cupola shall `fixing` 状態へ遷移するイベントを生成する
3. When conflictを検知した, the cupola shall 対象ブランチ名（head branch・base branch）を `.cupola/inputs/conflict_info.txt` としてworktreeに書き出す
4. If `mergeable` フィールドが `null`（GitHub側でまだ計算中）の場合, the cupola shall その判定をスキップして次のサイクルで再確認する
5. The cupola shall conflict情報ファイルにhead branch名・base branch名・デフォルトブランチ名を含める

### Requirement 3: 判定の優先順位制御

**Objective:** As a cupola利用者, I want `review_waiting` 状態のポーリングステップで複数の状態を正しい優先順位で判定する, so that 最も重要な状態変化が確実に処理される

#### Acceptance Criteria

1. The cupola shall `review_waiting` 状態のpollingサイクルにおいて以下の優先順位で判定を行う: (1) merge検知 → `completed`/次フェーズ、(2) CI失敗 → `fixing`、(3) conflict → `fixing`、(4) 未解決のreview thread → `fixing`、(5) 該当なし → 次サイクルへ
2. When merge済みであることを検知した, the cupola shall CI失敗・conflict・reviewスレッドの確認を行わず直ちに`completed`/次フェーズへ遷移する
3. When CI失敗を検知した, the cupola shall conflictおよびreviewスレッドの確認を継続して複数問題を同時に収集する
4. When conflictを検知した, the cupola shall reviewスレッドの確認を継続して複数問題を同時に収集する
5. If 複数の問題（CI失敗・conflict・未解決reviewスレッド）が同時に存在する, the cupola shall それらを全て収集して1回の `fixing` 遷移にまとめる

### Requirement 4: fixingプロンプトの動的組み立て

**Objective:** As a cupola利用者, I want 問題の種類に応じたfixingプロンプトをClaude Codeに送る, so that Claude Codeが問題の種類を把握して適切な修正を行える

#### Acceptance Criteria

1. The cupola shall 問題の種類（`review_comments` / `ci_failure` / `conflict`）を引数として受け取り、動的にfixingプロンプトを組み立てる
2. When `review_comments` 問題が含まれる, the cupola shall プロンプトに「`.cupola/inputs/review_threads.json` を参照して修正してください」という指示を含める
3. When `ci_failure` 問題が含まれる, the cupola shall プロンプトに「`.cupola/inputs/ci_errors.txt` を参照して修正してください」という指示を含める
4. When `conflict` 問題が含まれる, the cupola shall プロンプトに「`origin/{default_branch}` を取り込んでconflictを解消してください」という指示を含める
5. If 複数の問題が同時に存在する, the cupola shall 各問題の指示を全て含んだ単一のfixingプロンプトを生成する
6. The cupola shall 現在の静的な `build_fixing_prompt` 関数を、問題種別リストを受け取る動的生成方式へ変更する

### Requirement 5: fixing遷移時の入力ファイル整合性

**Objective:** As a cupola利用者, I want fixingへ遷移する際に必要な入力ファイルが確実に用意される, so that Claude Codeが修正に必要な情報にアクセスできる

#### Acceptance Criteria

1. When CI失敗により `fixing` へ遷移する, the cupola shall `.cupola/inputs/ci_errors.txt` が存在することを確認してからClaude Codeを起動する
2. When conflictにより `fixing` へ遷移する, the cupola shall `.cupola/inputs/conflict_info.txt` が存在することを確認してからClaude Codeを起動する
3. The cupola shall 入力ファイルへの書き出しは `fixing` 状態への遷移前に完了する
4. If ファイルの書き出しに失敗した, the cupola shall エラーをログに記録し `fixing` 遷移を中止する（`review_waiting` を維持する）
5. The cupola shall `fixing` が正常完了（Claude Codeが0終了）した場合はretry_countを増加させない
