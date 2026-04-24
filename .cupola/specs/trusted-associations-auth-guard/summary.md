# trusted-associations-auth-guard サマリ

## Feature
公開リポジトリでのプロンプトインジェクション攻撃対策として、GitHub の `author_association` を用いた**入力元認証**のセキュリティガードを実装。`cupola.toml` の `trusted_associations` 設定で信頼レベルを制御し、(1) `agent:ready` ラベル付与者の association チェック、(2) PR レビューコメントの `author_association` フィルタリング、の 2 経路で Claude Code に信頼済み入力のみを渡す。

## 要件サマリ
- **設定 (Req 1)**: `trusted_associations` を `cupola.toml` で指定。デフォルト `["OWNER", "MEMBER", "COLLABORATOR"]`。`["all"]` でチェックスキップ。有効値: `OWNER/MEMBER/COLLABORATOR/CONTRIBUTOR/FIRST_TIMER/FIRST_TIME_CONTRIBUTOR/NONE`。無効値は起動時にエラーで停止。
- **ラベル付与者チェック (Req 2)**: `agent:ready` 検出時に Timeline API で付与 actor.login を取得、Permission API で association を判定、信頼外ならラベル削除 + コメント通知。API 失敗時は**安全側でワークフロー停止**。結果を `tracing::info!` に記録。
- **レビューコメントフィルタ (Req 3)**: `review_threads.json` 書き出し時に各コメントの `author_association` を確認、信頼外は除外、除外数をログ記録。`["all"]` は全コメント通過。
- **ドキュメント (Req 4)**: `SECURITY.md`（プロンプトインジェクション説明 + `trusted_associations` 解説）、README に Fork ワークフロー + 設定例。

## アーキテクチャ決定
- **ドメイン型 `AuthorAssociation` enum + `TrustedAssociations` sum type (`All` / `Specific(Vec<AuthorAssociation>)`)** (採用): 型安全でパターンマッチが網羅的。`Vec<String>` のまま運用案は typo リスクとクリーンアーキテクチャ違反で不採用。既存 `ModelConfig` パターンと一貫。
- **Timeline API + Permission API の 2 段階取得** (採用): Timeline の `labeled` イベントには `author_association` が含まれない制約を発見。代替として `GET /repos/{owner}/{repo}/issues/{n}/timeline` で actor.login を取得 → `GET /repos/{owner}/{repo}/collaborators/{username}/permission` で permission (`admin/maintain/write/triage/read`) を取得し `RepositoryPermission` enum へ → `to_author_association()` で変換。OWNER 判定は `admin` permission + リポジトリオーナー login 一致、CONTRIBUTOR 以下は fallback で NONE 扱い（安全側）。
- **レビューコメントフィルタは application 層 (`io.rs`)** (採用): フィルタリングはビジネスロジックであり adapter 層に漏らすべきでない。Config 参照は application 層以上の権利。GraphQL 側で全取得→application でフィルタする若干のオーバーヘッドは許容。
- **GraphQL は `reviewThreads.comments` に `authorAssociation` フィールドを追加するだけ**: GitHub GraphQL `PullRequestReviewComment.authorAssociation` が直接 enum を返すため、REST の 2 段階取得と異なり軽量。
- **association チェックは `association_guard` モジュールに集約**: `PollingUseCase::step1_issue_polling` が `Event::IssueDetected` を push する前に呼ぶ。
- **Fail Safe 原則**: API 失敗はワークフロー続行しない。`author_association` 取得失敗（GraphQL）は `None` 扱いでデフォルト設定下では拒否。`["all"]` は意図的な明示設定を要求しデフォルトでは有効。
- **ログ粒度**: actor.login は記録、コメント本文は記録せず（プロンプトインジェクション内容の露出防止）。
- **非スコープ**: コンテンツサニタイズ、GitHub Actions 権限制御、CONTRIBUTOR/FIRST_TIMER の完全サポート（PR 履歴 API 追加が必要）。

## コンポーネント
- **domain**:
  - `AuthorAssociation` enum (`Owner/Member/Collaborator/Contributor/FirstTimer/FirstTimeContributor/None`)、`from_str` (大文字小文字不問)、`as_str`。
  - `TrustedAssociations` enum (`All` / `Specific(Vec<AuthorAssociation>)`)、`is_trusted`、`default()`。
  - `Config` に `trusted_associations` フィールド追加。
- **application**:
  - `association_guard` モジュール: `check_label_actor` + `AssociationCheckResult { Trusted, Rejected { actor_login, association } }`。`All` は即 `Trusted`。
  - `GitHubClient` trait 拡張: `fetch_label_actor_login`, `fetch_user_permission`, `remove_label`, `add_issue_comment`。
  - `RepositoryPermission` enum (`Admin/Maintain/Write/Triage/Read`) + `to_author_association()`。
  - `io::write_review_threads_input` に `trusted_associations` パラメータ追加、除外コメント数をログ。
  - `PollingUseCase::step1_issue_polling`: `Event::IssueDetected` push 前に `check_label_actor` 呼び出し。
- **adapter/outbound**:
  - `OctocrabRestClient`: Timeline API / Permission API 実装。404 は `Read` (NONE 相当)。
  - `GraphQLClient`: `reviewThreads` クエリに `authorAssociation` 追加、`ReviewComment.author_association` にマッピング。
- **bootstrap**:
  - `CupolaToml`: `trusted_associations: Option<Vec<String>>` 追加、`["all"]` 特別扱い、`Specific` 変換、`Config::validate()` で不正値検証。
- **ドキュメント**: `SECURITY.md` 新規、README に Fork ワークフロー + `cupola.toml` 設定例追記。

## 主要インターフェース
```rust
pub enum AuthorAssociation { Owner, Member, Collaborator, Contributor, FirstTimer, FirstTimeContributor, None }
pub enum TrustedAssociations { All, Specific(Vec<AuthorAssociation>) }
impl TrustedAssociations { pub fn is_trusted(&self, a: &AuthorAssociation) -> bool; }

pub enum AssociationCheckResult {
    Trusted,
    Rejected { actor_login: String, association: AuthorAssociation },
}
pub async fn check_label_actor<G: GitHubClient>(
    github: &G, issue_number: u64, label_name: &str,
    trusted: &TrustedAssociations, owner: &str, repo: &str,
) -> anyhow::Result<AssociationCheckResult>;

trait GitHubClient {
    fn fetch_label_actor_login(&self, issue_number: u64, label_name: &str)
        -> impl Future<Output = Result<Option<String>>>;
    fn fetch_user_permission(&self, username: &str)
        -> impl Future<Output = Result<RepositoryPermission>>;
    fn remove_label(&self, issue_number: u64, label_name: &str)
        -> impl Future<Output = Result<()>>;
    fn add_issue_comment(&self, issue_number: u64, body: &str)
        -> impl Future<Output = Result<()>>;
}
```

## 学び / トレードオフ
- **サニタイズではなく認証で防御**: プロンプトインジェクションは文字列パターンでは完全防御が不可能。入力「内容」ではなく「送信者」の信頼度で判定する設計判断は、攻撃面を大幅に縮小する正しいアプローチ。
- Timeline API の `labeled` イベントに `author_association` がない制約を研究段階で発見し、Permission API 組み合わせという代替経路を設計。REST (2 段階) と GraphQL (1 段階) で取得経路が非対称になった技術負債は許容。
- OWNER 判定が `collaborators/permission` 単独でできず、`admin` permission + リポジトリオーナー login 一致というヒューリスティックに頼る点は不完全だが、デフォルト設定（OWNER/MEMBER/COLLABORATOR）では実用上十分。
- `["all"]` を明示設定で有効化し、デフォルトを最も厳しくする Secure by Default 設計。プライベートリポジトリ向けのエスケープハッチを残しつつ、公開リポジトリでは誤設定を起こしにくい。
- API 失敗時に続行せずワークフロー停止する判断は、可用性を犠牲にしてセキュリティを優先（Fail Safe）。ラベル付与は低頻度なのでレート制限への影響は軽微。
- ログに actor.login を記録しコメント本文を記録しない粒度設計は、監査性とプロンプトインジェクション内容露出防止のバランスを取った判断。
- `ReviewComment` 構造体変更が既存テストに波及するリスクは、`AuthorAssociation::None` デフォルト値で後方互換を確保。
