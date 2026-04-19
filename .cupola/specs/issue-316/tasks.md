# 実装タスクリスト

## タスク概要

全体の依存関係:
1. **タスク 1**（ドメインモデル）を最初に完了させる
2. タスク 1 の完了後、**タスク 2・3・5** が並列実行可能
3. タスク 2 の完了後、**タスク 4・6** が並列実行可能
4. すべてのタスクの完了後、**タスク 7**（テスト強化）を実施

---

- [ ] 1. Issue / MetadataUpdates へのフラグ追加（ドメイン層）
- [ ] 1.1 `Issue` 構造体に `ci_fix_limit_notified: bool` を追加する
  - `pub ci_fix_limit_notified: bool` を `Issue` 構造体フィールドとして追加
  - `Issue::new` の初期化で `ci_fix_limit_notified: false` をセット
  - `Issue` のテスト内の struct リテラルに `ci_fix_limit_notified: false` を追加（コンパイルエラー解消）
  - _Requirements: 1.1, 1.2_

- [ ] 1.2 `MetadataUpdates` 構造体に `ci_fix_limit_notified: Option<bool>` を追加する
  - フィールド `pub ci_fix_limit_notified: Option<bool>` を追加（デフォルト `None`）
  - `apply_to` メソッドに `if let Some(v) = self.ci_fix_limit_notified { issue.ci_fix_limit_notified = v; }` を追加
  - `MetadataUpdates` のユニットテスト内の struct リテラルにフィールドを追加
  - _Requirements: 1.3, 1.4_

---

- [ ] 2. DB スキーマ・SQLite リポジトリの更新（タスク 1 完了後）
- [ ] 2.1 `CREATE TABLE issues` スキーマ定義に `ci_fix_limit_notified` 列を追加する
  - `sqlite_connection.rs` の `CREATE TABLE IF NOT EXISTS issues` 文に `ci_fix_limit_notified INTEGER NOT NULL DEFAULT 0,` を追加
  - _Requirements: 2.1_

- [ ] 2.2 既存 DB 向けのマイグレーションを追加する
  - `init_schema` 内の `run_add_column_migration` 呼び出し列に `"ci_fix_limit_notified INTEGER NOT NULL DEFAULT 0"` を追加
  - 既存 DB（列なし状態）でも `init_schema` が正常完了することを確認
  - _Requirements: 2.2_

- [ ] 2.3 (P) `SELECT` クエリと `row_to_issue` 関数に `ci_fix_limit_notified` を追加する
  - `find_by_id`, `find_by_issue_number`, `find_active`, `find_all`, `find_by_state` の各 SELECT 文の列リストに `ci_fix_limit_notified` を追加
  - `row_to_issue` 関数（またはクロージャ）で `ci_fix_limit_notified` 列を読み取り、`!= 0` で `bool` に変換して `Issue` にセット
  - _Requirements: 2.3_

- [ ] 2.4 (P) `save` (INSERT) に `ci_fix_limit_notified` を追加する
  - `save` メソッドの `INSERT INTO issues` 文の列リストと `params!` マクロに `ci_fix_limit_notified` を追加
  - 値は `issue.ci_fix_limit_notified as i64`
  - _Requirements: 2.4_

- [ ] 2.5 (P) `update_state_and_metadata` に `ci_fix_limit_notified` の動的 UPDATE 対応を追加する
  - `updates.ci_fix_limit_notified` が `Some(v)` の場合に `rusqlite::types::Value::Integer(v as i64)` を `param_values` に push し SET 句を生成するブロックを追加
  - _Requirements: 2.5_

---

- [ ] 3. Decide ロジックの発火条件変更（タスク 1 完了後、タスク 2 と並列可）
- [ ] 3.1 (P) CI failure 経路の発火条件を `>= max && !notified` に変更する
  - `decide_implementation_review_waiting` 内の CI failure 経路（旧 line 681）の条件を `prev.ci_fix_count >= cfg.max_ci_fix_cycles && !prev.ci_fix_limit_notified` に変更
  - `metadata_updates.ci_fix_limit_notified` は設定しない（execute が担う）
  - _Requirements: 3.1, 3.2, 3.4_

- [ ] 3.2 (P) conflict 経路の発火条件を同様に変更する
  - conflict 経路（旧 line 693）の条件を同様に `prev.ci_fix_count >= cfg.max_ci_fix_cycles && !prev.ci_fix_limit_notified` に変更
  - `metadata_updates.ci_fix_limit_notified` は設定しない
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

---

- [ ] 4. Execute での成功時フラグ書き込み（タスク 2 完了後）
- [ ] 4.1 `PostCiFixLimitComment` ハンドラを match 式に書き換え、成功時のみ `ci_fix_limit_notified = true` を永続化する
  - 現行の `github.comment_on_issue(n, &msg).await?` を `match github.comment_on_issue(n, &msg).await { Ok(()) => ..., Err(e) => ... }` に変更
  - 成功時: `issue_repo.update_state_and_metadata(issue.id, MetadataUpdates { ci_fix_limit_notified: Some(true), ..Default::default() }).await?` を呼ぶ
  - 失敗時: `tracing::warn!(issue_number = n, error = %e, "failed to post ci-fix-limit comment, will retry next cycle")` を記録し、エラーを外部に伝播しない（best-effort）
  - `execute` 関数のシグネチャに `issue_repo` が含まれていることを確認し、含まれていなければ引数として追加
  - _Requirements: 4.1, 4.2, 4.3_

---

- [ ] 5. status コマンドへの警告表示（タスク 1 完了後、タスク 3/4 と並列可）
- [ ] 5.1 (P) `handle_status` で `ci_fix_limit_notified` が未設定の Issue に警告を表示する
  - `handle_status` の Issue 表示ループ内で `issue.ci_fix_count > config.max_ci_fix_cycles && !issue.ci_fix_limit_notified` の条件を評価
  - 条件が true の場合、その Issue のエントリに `⚠ ci-fix-limit notification pending` を付加して表示
  - 条件が false の場合は何も追加しない
  - `config` がスコープ内にない場合は引数として渡せるよう調整する
  - _Requirements: 5.1, 5.2_

---

- [ ] 6. doctor コマンドへの警告チェック追加（タスク 2 完了後）
- [ ] 6.1 `DoctorUseCase` に `IssueRepository` 型パラメータと `max_ci_fix_cycles` を追加する
  - `DoctorUseCase<C, R, I: IssueRepository>` に型パラメータ `I` を追加
  - 構造体フィールドに `issue_repo: I` と `max_ci_fix_cycles: u32` を追加
  - `DoctorUseCase::new` の引数に `issue_repo: I` と `max_ci_fix_cycles: u32` を追加
  - `pub fn run` を `pub async fn run` に変更
  - bootstrap 側の `DoctorUseCase::new` 呼び出しと `run` 呼び出しを更新
  - _Requirements: 6.4_

- [ ] 6.2 `check_pending_ci_fix_limit_notifications` チェックを実装して `run` に追加する
  - `async fn check_pending_ci_fix_limit_notifications(issue_repo, max_ci_fix_cycles) -> DoctorCheckResult` を実装
  - `issue_repo.find_all().await` で全 Issue を取得し `ci_fix_count > max_ci_fix_cycles && !ci_fix_limit_notified` の件数を集計
  - 件数 > 0 なら `DoctorSection::OperationalReadiness`・`CheckStatus::Warn` を返し、remediation を "GitHub の通信状況を確認の上、`cupola start` で再度ポーリングを実行してください" とする
  - 件数 = 0 なら `DoctorSection::OperationalReadiness`・`CheckStatus::Ok` を返す
  - `DoctorUseCase::run` 内で `check_pending_ci_fix_limit_notifications` を呼び出し結果を `vec!` に追加
  - _Requirements: 6.1, 6.2, 6.3_

---

- [ ] 7. テスト強化
- [ ] 7.1 Decide ユニットテストを追加する
  - `count = max+1, notified = false` → `PostCiFixLimitComment` が発火するシナリオ（CI failure 経路）
  - `count = max+1, notified = false` → `PostCiFixLimitComment` が発火するシナリオ（conflict 経路）
  - `notified = true` → `PostCiFixLimitComment` が発火しないシナリオ
  - `count < max, notified = false` → `PostCiFixLimitComment` が発火しないシナリオ
  - _Requirements: 3.1, 3.2_

- [ ] 7.2 (P) Execute ユニットテストを追加する
  - mock `IssueRepository` を使って、GitHub 投稿成功時に `update_state_and_metadata` が `ci_fix_limit_notified: Some(true)` で呼ばれることを確認
  - mock `GitHubClient` が失敗を返す場合に `update_state_and_metadata` が呼ばれないことを確認
  - _Requirements: 4.1, 4.2_

- [ ] 7.3 (P) SQLite 統合テストを追加する
  - インメモリ DB で `init_schema` 後に `ci_fix_limit_notified` 列が存在することを確認
  - 列なし状態の DB に `init_schema` を適用し、マイグレーションが正常完了することを確認
  - `save` → `find_by_id` のラウンドトリップで `ci_fix_limit_notified` が正しく保持されることを確認
  - `update_state_and_metadata` で `ci_fix_limit_notified: Some(true)` を書き込み、`find_by_id` で取得できることを確認
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] 7.4 (P) DoctorUseCase ユニットテストを追加する
  - pending 件数 > 0 の場合に `CheckStatus::Warn` が返ることを確認（mock IssueRepository 使用）
  - pending 件数 = 0 の場合に `CheckStatus::Ok` が返ることを確認
  - `DoctorSection::OperationalReadiness` に属することを確認
  - remediation メッセージに `cupola start` が含まれることを確認
  - _Requirements: 6.1, 6.2, 6.3_
