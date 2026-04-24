# ci-conflict-fixing-trigger

## Feature
`review_waiting` 状態で PR の CI 失敗・merge conflict を自動検知し、問題種別に応じた入力ファイルとプロンプトを準備のうえ `fixing` 状態へ遷移させる拡張。既存は review thread の有無と merge 状態のみを監視しており、CI/conflict 問題が自動修正サイクルに乗らなかった課題に対応する。

## 要件サマリ
- `review_waiting` で polling 毎に GitHub Checks API と PR mergeable フィールドを確認し、CI 失敗・conflict・未解決 review を検知する。
- 優先順位: (1) merge → 完了、(2) CI 失敗、(3) conflict、(4) review thread、(5) 該当なし。
- 複数問題は 1 回の `fixing` 遷移にまとめる（`Issue.fixing_causes` に全原因を格納）。
- 問題別の入力ファイルを worktree に書き出す: `ci_errors.txt` / `conflict_info.txt` / 既存 `review_threads.json`。
- `build_fixing_prompt` を問題種別リスト引数の動的生成に変更し、各原因ごとの指示を単一プロンプトに結合。
- API 失敗時はサイクルをスキップし状態遷移しない。書き出し失敗時は `fixing` 遷移を中止し `review_waiting` を維持。
- 追加 API 呼び出しは issue 毎サイクル +2 回以内（Checks / PR mergeable）。

## アーキテクチャ決定
- **問題種別の表現**: `String` や bit flag ではなく `domain/fixing_problem_kind.rs` に `FixingProblemKind` enum（ReviewComments / CiFailure / Conflict）を追加。exhaustive match と serde JSON 変換で型安全性を確保。
- **複数問題の伝達方式**: 3 案検討。(A) `Issue.fixing_causes: Vec<FixingProblemKind>` フィールド追加、(B) Event に cause を埋め込む、(C) 入力ファイル有無で判断。(A) を採用。理由: ドメインの自然な拡張で state machine 変更を最小化、既存 Issue 管理フローに統合しやすい。DB スキーマは `ALTER TABLE issues ADD COLUMN fixing_causes TEXT NOT NULL DEFAULT '[]'` で移行。(B) は state machine と大量テストを書き換える必要があり不採用、(C) はファイルシステム依存で抽象が崩れるため不採用。
- **CI Checks 取得**: octocrab の check_runs 対応が限定的なため、既存 `GraphQLClient` と同様に `OctocrabRestClient` に `reqwest::Client` と `token` を追加し直接 REST を叩く。PR head SHA 取得 → check-runs 取得の 2 段階構成。
- **PR mergeable**: `GET /repos/.../pulls/{n}` で取得、`null`（計算中）はスキップし次サイクルで再確認。
- **イベント**: 新規 `FixingRequired` を導入せず、既存 `UnresolvedThreadsDetected` を汎用 fixing トリガーとして再利用（原因は `Issue.fixing_causes` 側に保持）。state machine 変更を最小化するため。
- **CI エラーログの保持場所**: `Issue` エンティティには原因種別のみ保存し、エラー本文は `prepare_inputs` 時点で GitHub から再取得して直接 `ci_errors.txt` に書き出す。DB 肥大化を避けるための判断。
- **プロンプト動的生成**: 静的 `build_fixing_prompt` を廃し、`causes: &[FixingProblemKind]` を受け取って対応指示を結合する方式に変更。`build_session_config` にも `fixing_causes` 引数を追加。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `FixingProblemKind` enum | domain | 問題種別の型安全な表現 |
| `Issue.fixing_causes` | domain | 現在の fixing セッションが扱う原因リスト |
| `GitHubClient::get_ci_check_runs` | application/port | Checks API ポート |
| `GitHubClient::get_pr_mergeable` | application/port | PR mergeable ポート |
| `step4_pr_monitoring`（更新） | application | 優先順位に沿った検知と causes 収集 |
| `prepare_inputs`（更新） | application | causes に応じた入力ファイル準備 |
| `build_fixing_prompt`（更新） | application/prompt | 問題種別別指示の動的組み立て |
| `write_ci_errors_input` / `write_conflict_info_input` | application/io | worktree への入力ファイル書き出し |
| `OctocrabRestClient`（拡張） | adapter/outbound | Checks / PR mergeable の REST 実装 |
| `GitHubClientImpl` | adapter/outbound | 新ポートの委譲 |
| SQLite `issues.fixing_causes` カラム | infra | JSON 配列で永続化 |

## 主要インターフェース
- `get_ci_check_runs(pr_number) -> Vec<GitHubCheckRun>`（`status=="completed"` のみ返却）
- `get_pr_mergeable(pr_number) -> Option<bool>`（`None`=計算中）
- `build_fixing_prompt(issue_number, pr_number, language, causes: &[FixingProblemKind]) -> String`
- `build_session_config(..., fixing_causes: &[FixingProblemKind])`
- `write_ci_errors_input(worktree, &[CiErrorEntry])` → `.cupola/inputs/ci_errors.txt`
- `write_conflict_info_input(worktree, &ConflictInfo { head_branch, base_branch, default_branch })` → `.cupola/inputs/conflict_info.txt`
- Prompt 指示例: ReviewComments → `review_threads.json` 参照、CiFailure → `ci_errors.txt` 参照、Conflict → `origin/{default_branch}` を取り込んで解消。

## 学び / トレードオフ
- `step6_apply_events` は issue あたり 1 サイクル 1 イベントしか処理しないため、複数問題をイベント多重発行では伝えられず、Issue 側に状態を持たせる必要があった。
- `mergeable: null` の非同期計算挙動を明示的にテスト対象にすべき。`false` との取り違えはユーザー体験を大きく損なう。
- DB マイグレーションは `PRAGMA table_info` で冪等に判定、または `ALTER TABLE` の重複エラーを握りつぶす方式でゼロダウンタイム対応。
- CI エラー本文を Issue 保持しないことで DB 肥大化を回避したが、`prepare_inputs` 時点で GitHub を再度叩くコストが発生する。rate limit 余裕内と判断。
- `UnresolvedThreadsDetected` を汎用 fixing トリガーとして流用する命名の不整合は許容。Event 再命名は波及範囲が大きいため別課題。
- CI 再実行や conflict 解消戦略の自動選択はスコープ外。Claude Code 側に委ねる。
