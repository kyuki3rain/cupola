# Research & Design Decisions

---
**Purpose**: 設計調査の記録と、design.md における主要な設計判断の根拠を記録する。
---

## Summary
- **Feature**: `ci-conflict-fixing-trigger`
- **Discovery Scope**: Extension（既存のpollingループ・fixing機構への追加）
- **Key Findings**:
  - `step4_pr_monitoring` に CI/conflict チェックを追加する形での拡張が最小変更
  - 複数問題の同時収集には `Issue` エンティティへの `fixing_causes` フィールド追加が最適
  - `build_fixing_prompt` の動的化は引数 `&[FixingProblemKind]` を追加する形で実現可能

## Research Log

### GitHub Checks API（CI ステータス取得）
- **Context**: PR の CI 結果を取得するために必要な API の確認
- **Sources Consulted**: GitHub REST API docs - Check Runs
- **Findings**:
  - エンドポイント: `GET /repos/{owner}/{repo}/commits/{ref}/check-runs`
  - `ref` は PR の head SHA（`octocrab.pulls().get(pr_num)` で `.head.sha` を取得可能）
  - `conclusion` フィールドが `"failure"` のものを失敗とみなす
  - `status` が `"completed"` でない場合はまだ実行中 → 判定スキップ
  - octocrab には `check_runs` の直接サポートが不完全 → reqwest による直接 REST 呼び出しを使用（GraphQLClient と同様のパターン）
- **Implications**:
  - PR head SHA 取得 → check-runs 取得の2段階になる
  - `OctocrabRestClient` に `reqwest::Client` を追加、または `GraphQLClient` と同様に直接呼び出し用のクライアントを追加

### GitHub PR mergeable フィールド
- **Context**: PR のconflict状態を取得するための API の確認
- **Sources Consulted**: GitHub REST API docs - Pulls
- **Findings**:
  - `GET /repos/{owner}/{repo}/pulls/{number}` のレスポンスに `mergeable: bool | null` が含まれる
  - `null` はGitHub側がまだマージ可能性を計算中（非同期計算）→ スキップして次サイクルで再確認
  - `false` はconflictあり
  - 既存の `is_pr_merged` で使用している `octocrab.pulls().get(pr_num)` で同時に取得可能
- **Implications**:
  - 既存の `is_pr_merged` と同じ API 呼び出しで mergeable フィールドも取得できる
  - 新メソッド `get_pr_details(pr_number)` を追加して merged + mergeable を一度に返すか、別途 `get_pr_mergeable` を追加する

### 複数問題の同時収集アーキテクチャ
- **Context**: CI失敗・conflict・reviewスレッドが同時に存在する場合に1回のfixingにまとめる要件
- **Sources Consulted**: 既存の `polling_use_case.rs` のイベント処理フロー
- **Findings**:
  - 現状の `step6_apply_events` は1サイクルにつきIssueあたり最初の1イベントのみを処理（優先度順ソート後）
  - 複数の問題を「1回のfixing遷移」にまとめるには、問題種別をIssueエンティティに持たせるか、ステップ4内部で集約する必要がある
  - `Event::UnresolvedThreadsDetected` のようなイベントを複数発行しても、1サイクルでは1つしか処理されない
- **Implications**:
  - `Issue` エンティティに `fixing_causes: Vec<FixingProblemKind>` を追加する
  - step4でIssue更新（causes書き込み）→ イベント発行（`FixingRequired`）の順で処理
  - `build_session_config` は `issue.fixing_causes` を参照してプロンプトを動的生成

### 既存のpollingフローへの統合ポイント
- **Context**: 最小変更で要件を満たすための統合箇所の特定
- **Sources Consulted**: `polling_use_case.rs`, `prompt.rs`, `io.rs`, `event.rs`
- **Findings**:
  - `step4_pr_monitoring`: merge確認後にCI/conflict確認を追加するだけで優先順位制御が実現できる
  - `prepare_inputs`: `issue.fixing_causes` を見てCI errors/conflict infoを書き出すよう変更
  - `build_session_config`: `issue.fixing_causes` を引数として渡してpromptを動的生成
  - `Event` 列挙型に新しいvariantを追加するのは最小変更だが、state machineの遷移ルールとの整合性確認が必要
- **Implications**:
  - `Event::UnresolvedThreadsDetected` → `Event::FixingRequired` に統一（cause情報を含まないシンプルなevent）
  - OR: `UnresolvedThreadsDetected` は残してCI/conflict検知も同じイベントにマップ
  - Issue DBスキーマ変更（`fixing_causes` カラム追加）と移行が必要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: Issue.fixing_causes フィールド追加 | Issueエンティティに問題種別リストを持たせる | 複数問題の同時収集を自然に表現。既存アーキテクチャに合致 | DBスキーマ変更が必要 | 採用 |
| B: イベントにcause情報を埋め込む | Event::UnresolvedThreadsDetected を Event::FixingRequired { causes } に変更 | イベントのみで情報が完結 | state machine変更・既存テスト大量修正が必要 | 不採用（変更範囲が広すぎる） |
| C: 入力ファイルの存在で判断 | 書き出した入力ファイルの有無で問題種別を判断 | DBスキーマ変更不要 | ファイルシステム状態への依存が増える。クリーンな抽象が崩れる | 不採用 |

## Design Decisions

### Decision: `FixingProblemKind` 列挙型をdomain層に追加

- **Context**: 問題種別（review_comments/ci_failure/conflict）を型安全に表現する必要がある
- **Alternatives Considered**:
  1. `String` で表現 — 型安全性がなく、typoリスクがある
  2. `u8` フラグビット — 表現力が低い
- **Selected Approach**: `domain/` に `FixingProblemKind` enum を追加（serde derive付き）
- **Rationale**: ドメインの概念を正確に表現。exhaustive matchingで網羅性保証。JSON serialization対応
- **Trade-offs**: ドメイン層に新型が増えるが、コンセプトとして正当
- **Follow-up**: SQLite保存時はJSON配列としてシリアライズ

### Decision: `Issue` エンティティへの `fixing_causes` フィールド追加

- **Context**: 複数問題を1回のfixingにまとめるために、どのメカニズムで問題種別を伝達するか
- **Alternatives Considered**:
  1. Event に情報を埋め込む — イベント定義とstate machine変更が広範
  2. 入力ファイルの存在を確認 — ファイルシステム依存が増える
- **Selected Approach**: `Issue.fixing_causes: Vec<FixingProblemKind>` としてDBに保存
- **Rationale**: Issueエンティティが「現在のfixingに何が必要か」を知っているのは自然。既存のIssue管理フローに自然に統合できる
- **Trade-offs**: DBスキーマ変更（`ALTER TABLE issues ADD COLUMN fixing_causes TEXT DEFAULT '[]'`）が必要
- **Follow-up**: SQLite移行スクリプトまたはApplication起動時のスキーマ自動適用

### Decision: CI Check Runs の取得方法

- **Context**: octocrab の Checks API サポートが限定的
- **Alternatives Considered**:
  1. octocrab の check_runs メソッドを使用
  2. reqwest による直接 REST 呼び出し（GraphQLClient のパターン踏襲）
- **Selected Approach**: reqwest を使用した直接 REST 呼び出し（`OctocrabRestClient` に `reqwest::Client` と `token` フィールドを追加）
- **Rationale**: 既存の GraphQLClient が reqwest を使用しており、同パターンで一貫性が保てる
- **Trade-offs**: octocrab の型安全な API を使わないが、serde_json::Value parsing で対応可能
- **Follow-up**: rate limit は step4 あたり最大+2回/Issue で許容範囲

## Risks & Mitigations
- DBスキーマ変更によるマイグレーション — `init_schema` に `IF NOT EXISTS` + `ALTER TABLE` 追加でゼロダウンタイム対応
- GitHub `mergeable: null` の扱い — スキップ判定を明確にテスト
- CI check-runs API の rate limit — 既存の polling interval 範囲内（+2 calls/issue/cycle）で問題なし
- 複数問題が同時発生した際のfixing_causes 上書き — step4 では毎サイクル新たに判定・上書きするため冪等

## References
- [GitHub Checks API - List check runs for a Git reference](https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference)
- [GitHub Pulls API - Get a pull request](https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request) — mergeable フィールド
