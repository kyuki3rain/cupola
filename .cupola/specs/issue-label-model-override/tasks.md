# 実装計画

## タスク一覧

- [ ] 1. (P) Issue エンティティと DB スキーマを model カラム向けに拡張する
- [ ] 1.1 (P) Issue エンティティに model フィールドを追加する
  - `domain/issue.rs` の `Issue` struct に `model: Option<String>` フィールドを追加する
  - `None` はモデル未指定（ラベルなし）、`Some(name)` はラベルから取得したモデル名を表す
  - ドメイン層はフォーマット検証を行わず、純粋なデータ保持のみを担う
  - _Requirements: 1.1, 1.2, 1.4, 4.1_

- [ ] 1.2 (P) issues テーブルに model カラムを追加し、マイグレーションを実装する
  - `sqlite_connection.rs` の `CREATE TABLE IF NOT EXISTS issues` 定義に `model TEXT` カラムを追加する
  - 既存 DB に対してカラムが存在しない場合のみ `ALTER TABLE issues ADD COLUMN model TEXT` を実行するマイグレーション処理を `init` フェーズに追加する
  - マイグレーション処理はべき等（すでにカラムが存在する場合はスキップ）にする
  - _Requirements: 1.4, 4.1_

- [ ] 1.3 IssueRepository の SQL クエリを model カラム対応にする
  - `SqliteIssueRepository` の SELECT クエリ（`find_by_id`・`find_active`・`find_needing_process`）に `model` カラムを追加し、`Option<String>` にマッピングする
  - `save`（INSERT）クエリに `model` カラムを追加する
  - `update` クエリに `model` カラムを追加する
  - `NULL` は `Option::None`、値がある場合は `Option::Some(model_name)` として扱う
  - タスク 1.1 と 1.2 の完了後に実施すること
  - _Requirements: 1.1, 1.2, 1.4, 4.1_

- [ ] 2. (P) GitHub API からラベル一覧を取得する機能を追加する
- [ ] 2.1 (P) GitHubClient トレイトに get_issue_labels メソッドを追加する
  - `application/port/github_client.rs` の `GitHubClient` トレイトに `get_issue_labels(issue_number: u64) -> impl Future<Output = Result<Vec<String>>> + Send` メソッドを追加する
  - 戻り値はラベル名の文字列リスト（ラベルなしの場合は空 Vec）
  - GitHub API エラー時は `Err` を返す
  - タスク 1 と並行して実施可能（異なるファイルを操作するため）
  - _Requirements: 2.1_

- [ ] 2.2 OctocrabRestClient に get_issue_labels を実装する
  - `adapter/outbound` の GitHub クライアント実装に `get_issue_labels` を追加する
  - GitHub REST API の `GET /repos/{owner}/{repo}/issues/{issue_number}/labels` エンドポイントを呼び出す
  - レスポンスからラベル名を抽出し `Vec<String>` として返す
  - タスク 2.1 の完了後に実施すること
  - _Requirements: 2.1_

- [ ] 3. ポーリングサイクルにラベル動的更新とモデル優先順位解決を追加する
- [ ] 3.1 ラベル抽出とモデル優先順位解決のヘルパーを実装する
  - `PollingUseCase` に `extract_model_from_labels(labels: &[String]) -> Option<String>` プライベートメソッドを追加する
  - `model:` プレフィックスで始まるラベルを先頭から検索し、プレフィックスを除いた値を返す
  - `model:*` ラベルが複数存在する場合は最初に見つかったものを使用する
  - `PollingUseCase` に `resolve_model<'a>(issue_model: Option<&'a str>, config_model: &'a str) -> &'a str` プライベートメソッドを追加する
  - 優先順位: `issue_model` が `Some` → その値、`None` かつ `config_model` が空でない → `config_model`、それ以外 → `"sonnet"`
  - タスク 1 と 2 の完了後に実施すること
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 3.1, 3.2, 3.3, 3.4_

- [ ] 3.2 step1 のポーリングループに非終端 Issue のラベル再確認処理を追加する
  - `step1_issue_polling` 内の既存 `find_active` ループで、`is_issue_open` チェックと並行して `get_issue_labels` を呼び出す
  - 取得したラベルから `extract_model_from_labels` でモデル名を抽出し、`issue.model` と異なる場合のみ `issue_repo.update` を呼び出して DB を更新する
  - `get_issue_labels` が失敗した場合は `warn` ログを出力し、`issue.model` の更新をスキップして前回値を維持する
  - モデルが変更された場合は `info` ログ（`issue_number`・`model` フィールド付き）を出力する
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ] 3.3 step7 の Claude Code 起動処理にモデル優先順位解決を組み込む
  - `step7_spawn_processes` 内で `&self.config.model` を直接 `spawn` に渡している箇所を `resolve_model` を使った呼び出しに置き換える
  - `issue.model.as_deref()` と `&self.config.model` を `resolve_model` に渡し、解決済みのモデル名を `spawn` の引数に使用する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.3_

- [ ] 4. テストを追加して全体の動作を検証する
- [ ] 4.1 (P) ヘルパーメソッドのユニットテストを追加する
  - `extract_model_from_labels` のテスト:
    - `model:opus` ラベルが含まれる場合に `Some("opus")` を返すこと
    - 複数の `model:*` ラベルが含まれる場合は先頭の値を返すこと
    - `model:*` ラベルが存在しない場合は `None` を返すこと
  - `resolve_model` のテスト:
    - `issue_model = Some("opus")` の場合は `"opus"` を返すこと
    - `issue_model = None` かつ `config_model = "haiku"` の場合は `"haiku"` を返すこと
    - `issue_model = None` かつ `config_model = ""` の場合は `"sonnet"` を返すこと
  - _Requirements: 1.1, 1.2, 1.3, 3.1, 3.2, 3.3, 3.4_

- [ ] 4.2 (P) マイグレーションのテストを追加する
  - `model` カラムなしの既存 issues テーブルに対してマイグレーションを実行すると `model TEXT NULL` カラムが追加されること
  - マイグレーションをべき等に実行できること（2 回実行してもエラーにならないこと）
  - 既存レコードの `model` は `NULL` になること
  - _Requirements: 1.4, 4.1_

- [ ] 4.3 統合テストで end-to-end の動作を検証する
  - モック `GitHubClient` が `get_issue_labels` で `["model:opus", "agent:ready"]` を返す設定で `step1_issue_polling` を実行した場合、DB の `issue.model` が `"opus"` に更新されること
  - `model:*` ラベルを削除（空リストを返す）した後にポーリングサイクルを実行すると `issue.model` が `None` に更新されること
  - `issue.model = Some("opus")` の状態で `step7_spawn_processes` を実行すると、モック `ClaudeCodeRunner` が `"opus"` を受け取ること
  - `issue.model = None` かつ `config.model = "haiku"` の状態では `"haiku"` が渡されること
  - `issue.model = None` かつ `config.model = "sonnet"` の状態では `"sonnet"` が渡されること
  - 既存テストが全てパスすること（デグレなし）
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 4.2, 4.3_
