# issue-label-model-override サマリ

## Feature
GitHub Issue の `model:*` ラベルを読み取り、Claude Code 起動時のモデル（例: `opus`, `haiku`）を Issue ごとに動的に上書きする機能。失敗した Issue に後からラベルを付け替えるだけで別モデルで再実行できる運用を実現する。

## 要件サマリ
- `agent:ready` 検出時および毎ポーリングサイクル (step1) で `model:*` ラベルを再解析し、`issues.model` カラム (`Option<String>`) に反映する。
- 複数ある場合は先頭ラベルを採用。ラベル削除時は `NULL` にクリア。
- Claude Code 起動時 (step7) のモデル優先順位: `issues.model` → `cupola.toml` の `model` → デフォルト `"sonnet"`。
- 既存 DB・既存テストとの後方互換を維持する（`model` は nullable）。

## アーキテクチャ決定
- **GitHubClient に `get_issue_labels` を追加** (採用): 既存 `get_issue()` だと title/body まで取得して無駄、`list_ready_issues` の `GitHubIssue` はラベルを持たないため。専用・軽量 API が用途に最適。
- **優先順位解決は application 層 (PollingUseCase)**: モデル選択はユースケース固有のオーケストレーションで、ドメイン層を純粋に保ちたいため。
- **モデル名のバリデーションはしない**: サポートモデルは変化するためハードコードのメンテコストが高く、誤指定は Claude Code 起動失敗→retry で検知できる。
- **DB マイグレーションは `PRAGMA table_info` 確認後に `ALTER TABLE ADD COLUMN model TEXT`**: 既存 DB 互換のため nullable。
- **Fetch API レート**: 現時点の Issue 数では許容範囲と判断し、最適化は将来課題。

## コンポーネント
- `domain/issue.rs`: `Issue` に `model: Option<String>` 追加（純粋なデータ保持）。
- `application/port/github_client.rs`: `GitHubClient` トレイトに `get_issue_labels` 追加。
- `application/polling_use_case.rs`: step1 でラベル再確認・DB 更新、step7 で `resolve_model` によるモデル解決。プライベートヘルパー `extract_model_from_labels` / `resolve_model` を追加。
- `adapter/outbound/sqlite_issue_repository.rs`: SELECT/INSERT/UPDATE に `model` カラム対応。
- `adapter/outbound/sqlite_connection.rs`: CREATE TABLE 定義と起動時マイグレーション。
- `adapter/outbound/github_rest_client.rs` (OctocrabRestClient): `GET /repos/{owner}/{repo}/issues/{n}/labels` 実装。

## 主要インターフェース
```rust
trait GitHubClient {
    fn get_issue_labels(&self, issue_number: u64)
        -> impl Future<Output = Result<Vec<String>>> + Send;
}

impl PollingUseCase {
    fn extract_model_from_labels(labels: &[String]) -> Option<String>;
    fn resolve_model<'a>(issue_model: Option<&'a str>, config_model: &'a str) -> &'a str;
}
```
`ClaudeCodeRunner::spawn` は既存で `model: &str` 引数を持ち、変更不要。

## 学び / トレードオフ
- `ClaudeCodeRunner::spawn` が元々 `model` 引数を取っていたため spawn 側変更は最小で済み、既存設計の先見性が活かされた。
- step1 内でラベル取得を挟むことで API 呼び出しが Issue 数×サイクルに比例して増えるが、現状 Issue 数が少ないため許容。将来は `list_ready_issues` レスポンスにラベルを含める最適化余地あり。
- ラベル取得失敗時は `warn` ログのみで `model` を前回値維持とし、GitHub API 一時障害でモデル上書きがゼロに戻る事故を防いでいる。
- `model:*` ラベル複数存在時の順序は GitHub API 次第だが「先頭優先」を仕様として明示した。
