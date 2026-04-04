# リサーチ・設計決定ログ

---
**Purpose**: trusted-associations-auth-guard 機能の調査結果・設計根拠を記録する。

---

## Summary
- **Feature**: `trusted-associations-auth-guard`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `Config` 構造体（`src/domain/config.rs`）および `CupolaToml`（`src/bootstrap/config_loader.rs`）への新フィールド追加が最小変更で実現可能
  - GitHub Timeline REST API（`GET /repos/{owner}/{repo}/issues/{issue_number}/timeline`）でラベル付与者のログインを取得し、別途 Collaborator/Org API で association を判定する必要がある
  - GraphQL の `reviewThreads` クエリへ `authorAssociation` フィールドを追加するだけで PR フィルタ対応が完了する
  - `ReviewComment` 構造体（`src/application/port/github_client.rs`）に `author_association` フィールドを追加し、アプリケーション層でフィルタリングすることでクリーンアーキテクチャに準拠できる

---

## Research Log

### GitHub Timeline API と author_association の取得方法

- **Context**: `agent:ready` ラベルを付与したユーザーの association を取得する方法の選定
- **Sources Consulted**:
  - GitHub REST API ドキュメント: Issues > Timeline Events
  - GitHub REST API ドキュメント: Repos > Collaborators
- **Findings**:
  - `GET /repos/{owner}/{repo}/issues/{issue_number}/timeline` は `event: "labeled"` イベントを返し、`actor.login` が含まれる
  - Timeline イベントの `actor` オブジェクトには `author_association` フィールドが含まれない（Issues/PRs/Comments のみ）
  - association の判定は以下の API を組み合わせて実施する必要がある:
    1. リポジトリオーナーとの比較（`GET /repos/{owner}/{repo}` の `owner.login`）
    2. コラボレータ確認: `GET /repos/{owner}/{repo}/collaborators/{username}` → 204 なら COLLABORATOR 以上
    3. Org メンバー確認: `GET /orgs/{org}/members/{username}` → 204 なら MEMBER 以上
  - あるいは `octocrab` の `repos.list_collaborators()` を使い、permission level から OWNER/MEMBER/COLLABORATOR を判別できる
- **Implications**:
  - REST API 複数呼び出しのオーバーヘッドがある。ただし agent:ready ラベル付与は低頻度なので許容範囲
  - `author_association` という文字列を直接 API から取得できないため、ドメイン層で判定ロジックを持つ

### GraphQL ReviewThreads への author_association 追加

- **Context**: PR レビューコメントのフィルタリング実装
- **Sources Consulted**:
  - GitHub GraphQL API スキーマ（`PullRequestReviewComment.authorAssociation`）
  - 既存 `src/adapter/outbound/github_graphql_client.rs`
- **Findings**:
  - GraphQL の `PullRequestReviewComment` 型には `authorAssociation: CommentAuthorAssociation` フィールドが存在する
  - `CommentAuthorAssociation` は enum: `OWNER | MEMBER | COLLABORATOR | CONTRIBUTOR | FIRST_TIMER | FIRST_TIME_CONTRIBUTOR | NONE`
  - 既存クエリに `authorAssociation` を追加するだけで取得可能
- **Implications**:
  - `ReviewComment` 構造体に `author_association: AuthorAssociation` フィールドを追加するだけで対応できる
  - フィルタリングロジックはアプリケーション層（`io.rs`）に配置する

### Config への trusted_associations フィールド追加

- **Context**: TOML 設定からのフィールド読み込み
- **Sources Consulted**:
  - `src/domain/config.rs`（Config 構造体）
  - `src/bootstrap/toml_config_loader.rs`（CupolaToml）
- **Findings**:
  - `CupolaToml` は `serde::Deserialize` で TOML を読み込む。`Vec<String>` で `trusted_associations` を受け取り、ドメイン型に変換する
  - 既存の `model` → `ModelConfig` 変換パターンと同様の approach で実装可能
  - `"all"` は特別値として `TrustedAssociations::All` バリアントで表現する
- **Implications**:
  - `Config::validate()` で不正な association 文字列を検出しエラーを返す
  - デフォルト値は `TrustedAssociations::Specific(vec![Owner, Member, Collaborator])`

---

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ドメイン型 `TrustedAssociations` enum | `All` と `Specific(Vec<AuthorAssociation>)` の sum type | 型安全、パターンマッチで網羅的処理 | 変換ロジックが必要 | 既存 `ModelConfig` の WeightModelConfig パターンと一貫 |
| `Vec<String>` のまま運用 | 文字列で保持してチェック時に比較 | 実装コスト低 | 型安全性なし、typo のリスク | クリーンアーキテクチャの方針に反する |

## Design Decisions

### Decision: `author_association` の取得方式（Timeline Actor）

- **Context**: Timeline API の labeled イベントには `author_association` が含まれない
- **Alternatives Considered**:
  1. REST API 複数呼び出し（collaborators + org members チェック）
  2. GraphQL で Issue timeline を取得し actor の permission を判定
- **Selected Approach**: REST API で Timeline → actor.login を取得し、`GET /repos/{owner}/{repo}/collaborators/{username}/permission` API（単一エンドポイント）で permission を取得して association にマッピング
- **Rationale**: `collaborators/permission` エンドポイントは `permission` フィールドで `admin/maintain/write/triage/read` を返す。このエンドポイントは `author_association` を直接返すわけではないため、permission level を `RepositoryPermission` enum にマッピングし、ドメイン層で `AuthorAssociation` に変換する。OWNER 判定はこのエンドポイント単独では不確かなため、`admin` permission かつリポジトリオーナーの login と一致する場合に OWNER とする設計とする。CONTRIBUTOR 以下は fallback で NONE 扱いとして安全側に倒す
- **Trade-offs**: CONTRIBUTOR/FIRST_TIMER の完全判定には PR 履歴 API が必要だが、デフォルト設定（OWNER/MEMBER/COLLABORATOR）では不要
- **Follow-up**: CONTRIBUTOR 判定を将来サポートする場合は PR 検索 API 追加が必要

### Decision: フィルタリングの実施タイミング（PR レビューコメント）

- **Context**: review_threads フィルタをどの層で実施するか
- **Alternatives Considered**:
  1. Adapter 層でフィルタ（GraphQL クエリに where 句的な処理）
  2. Application 層でフィルタ（`write_review_threads_input` 呼び出し前）
- **Selected Approach**: Application 層（`io.rs` または呼び出し元の use case）でフィルタリング
- **Rationale**: フィルタリングはビジネスロジックであり、adapter 層に漏らすべきでない。Config を参照できるのはアプリケーション層以上
- **Trade-offs**: GraphQL で全コメントを取得するため若干データ転送量が増えるが、PR コメント数は通常少ないため問題ない

---

## Risks & Mitigations

- GitHub API レート制限: Timeline API + collaborators API の追加呼び出しがレート制限に引っかかる可能性 → ラベル付与イベントは低頻度なので影響は軽微
- `author_association` の網羅性: Timeline actor の association を完全に判定するのは複数 API 必要 → デフォルト設定（OWNER/MEMBER/COLLABORATOR）では `collaborators/permission` 1 回で対応可能
- 既存テストへの影響: `ReviewComment` 構造体変更により既存テストの修正が必要な場合あり → `author_association` は `Option<AuthorAssociation>` または `AuthorAssociation::None` デフォルト値で後方互換を確保

---

## References
- GitHub Timeline Events API: https://docs.github.com/en/rest/issues/timeline
- GitHub Repository Collaborator Permission API: https://docs.github.com/en/rest/collaborators/collaborators#get-repository-permissions-for-a-user
- GitHub GraphQL CommentAuthorAssociation: https://docs.github.com/en/graphql/reference/enums#commentauthorassociation
