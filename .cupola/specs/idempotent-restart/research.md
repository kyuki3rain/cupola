# Research & Design Decisions

---
**Purpose**: idempotent-restart フィーチャーの調査結果・設計判断の記録

---

## Summary
- **Feature**: `idempotent-restart`
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `reset_for_restart` は全フィールドをリセットするため、既存リソース（worktree・PR）が再利用できない
  - `initialize_issue` はすでに worktree が存在する場合でも `worktree.create()` を呼び出し、エラーになる可能性がある
  - `step7_spawn_processes` はスポーン前に DB の PR 番号を確認していないため、マージ済み PR があっても再実行される
  - `GitHubClient` トレイトには PR の open/closed/merged 状態を一度に取得するメソッドが存在しない（`is_pr_merged` と `get_pr_details` が分離）
  - `IssueRepository` には state で絞り込む `find_by_state` メソッドが存在しない（cleanup コマンドに必要）

## Research Log

### reset_for_restart の現在の実装

- **Context**: Cancelled からの reopen 時にリソースが再利用できるか確認
- **Sources Consulted**: `src/adapter/outbound/sqlite_issue_repository.rs`
- **Findings**:
  - SQL で `design_pr_number`, `impl_pr_number`, `worktree_path`, `feature_name`, `model` を全て NULL にリセットしている
  - `state='idle'`, `retry_count=0`, `current_pid=NULL`, `error_message=NULL` はリセットが必要
  - PR 番号・worktree パス・feature_name は次回再開時に再利用できる情報
- **Implications**: SQL の SET 句から `design_pr_number`, `impl_pr_number`, `worktree_path`, `feature_name` を削除するだけで対応可能。`model` は NULL にしても問題ない（次回検出時に config から再設定されるため）

### Cancelled + RetryExhausted 時の cleanup 処理

- **Context**: RetryExhausted で worktree を保持するために何を変更するか確認
- **Sources Consulted**: `src/application/transition_use_case.rs`
- **Findings**:
  - `(State::Cancelled, Event::RetryExhausted)` のアームで `self.cleanup(issue).await` が呼ばれている
  - `(State::Cancelled, Event::IssueClosed)` のアームでも `self.cleanup(issue).await` が呼ばれている
  - `cleanup()` は worktree 削除・ブランチ削除を実行する
- **Implications**: RetryExhausted アームから `self.cleanup(issue).await` を削除するだけ。Issue close は引き続き実行する

### initialize_issue の現在の実装

- **Context**: worktree が既存の場合に冪等に動作するか確認
- **Sources Consulted**: `src/application/polling_use_case.rs`（`initialize_issue` メソッド）
- **Findings**:
  - `self.worktree.create(wt, &main_branch, &start_point)` を無条件に呼び出している
  - `GitWorktree::create()` は既存 worktree があると失敗する可能性が高い
  - `worktree_path` を `issue.worktree_path = Some(wt_path)` でセットしているが、既存の場合はすでにセットされているはず
- **Implications**: `Path::new(&wt_path).exists()` で分岐し、存在する場合はスキップ。コメントメッセージは `design_starting` または `resuming_design` を条件で切り替える

### step7 スポーン前 PR チェックの設計

- **Context**: スポーン前に PR 状態を確認してスキップできるか検討
- **Sources Consulted**: `src/application/polling_use_case.rs`（`step7_spawn_processes`）、`src/application/port/github_client.rs`
- **Findings**:
  - `step7_spawn_processes` は `find_needing_process()` で取得した Issue に対してセッションをスポーンしている
  - `needing_process` は `DesignRunning`, `ImplementationRunning`, `DesignFixing`, `ImplementationFixing` 状態の Issue
  - `GitHubClient` に `is_pr_merged(pr_number)` はあるが、PR が open か closed(not merged) かを区別するメソッドがない
  - `get_pr_details()` は `merged: bool` と `mergeable: Option<bool>` を返すが、PR が open か closed かは含まれていない
- **Implications**: `GitHubClient` トレイトに `get_pr_status(pr_number: u64) -> Result<PrStatus>` を追加し、`PrStatus { Open, Closed, Merged }` で三状態を表現する

### cleanup コマンドの設計

- **Context**: `cupola cleanup` コマンドの実装方針を検討
- **Sources Consulted**: `src/adapter/inbound/cli.rs`, `src/bootstrap/app.rs`, `src/application/port/issue_repository.rs`
- **Findings**:
  - CLI は clap derive で `Command` enum に subcommand を追加する形式
  - `IssueRepository` に `find_by_state(state: State)` メソッドがない（`find_active()` は terminal state を除外している）
  - `GitWorktree` に `remove(path)` と `delete_branch(branch)` はある
  - `app.rs` で各コマンドに対応するユースケースをワイヤリングしている
- **Implications**: 
  - `IssueRepository` トレイトに `find_by_state` を追加
  - `CleanupUseCase` を `src/application/cleanup_use_case.rs` に新規作成
  - `Command::Cleanup` を `cli.rs` に追加
  - `app.rs` にワイヤリングを追加

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク・制限 | 備考 |
|-----------|------|------|--------------|------|
| PrStatus 列挙型を新設 | `GitHubClient` に `get_pr_status` メソッドを追加 | 三状態を型安全に表現、既存の `is_pr_merged` とロジックが明確に分離 | `GitHubClientImpl` に実装が必要 | **採用** |
| `get_pr_details` に state フィールド追加 | 既存の `GitHubPrDetails` に `state: String` を追加 | 変更が最小 | "open"/"closed" の文字列比較が必要、型安全性が低い | 不採用 |
| CleanupUseCase を独立ユースケースとして実装 | `src/application/cleanup_use_case.rs` に新規作成 | Clean Architecture に準拠、テストが容易 | ファイルが増える | **採用** |
| app.rs に cleanup ロジックを直接実装 | bootstrap 層に直接書く | シンプル | 責務混在、テスト困難 | 不採用 |

## Design Decisions

### Decision: `PrStatus` 列挙型の新設

- **Context**: step7 でスポーン前に PR 状態（merged/open/closed）を確認するために、GitHub API から三状態を取得する必要がある
- **Alternatives Considered**:
  1. `get_pr_details` に `state: String` フィールドを追加（"open"/"closed"）
  2. 新しい `PrStatus` enum と `get_pr_status` メソッドを追加
- **Selected Approach**: `PrStatus { Open, Closed, Merged }` enum を `github_client.rs` に定義し、`GitHubClient::get_pr_status(pr_number: u64) -> Result<PrStatus>` を追加
- **Rationale**: 型安全に三状態を表現でき、文字列比較によるバグを防ぐ。既存の `is_pr_merged` との役割分担も明確
- **Trade-offs**: 実装側（`GitHubClientImpl`）で REST API の `state` フィールドをマッピングする必要があるが、コード量は少ない
- **Follow-up**: `OctocrabRestClient` または `GitHubRestClient` での実装時に REST API レスポンスの `state` フィールドと `merged` フィールドを確認

### Decision: `IssueRepository::find_by_state` の追加

- **Context**: `cupola cleanup` で Cancelled 状態の Issue を取得するために、state で絞り込むメソッドが必要
- **Alternatives Considered**:
  1. `find_active()` を拡張して terminal state も返すオプションを追加
  2. 新しい `find_by_state(state: State) -> Result<Vec<Issue>>` を追加
- **Selected Approach**: `find_by_state` を追加
- **Rationale**: `find_active()` のセマンティクスを壊さずに、特定状態の Issue 取得を明示的に表現できる
- **Trade-offs**: ポートとアダプターの両方に実装が必要だが、変更は最小限
- **Follow-up**: モックアダプターにも実装が必要（統合テスト用）

### Decision: `initialize_issue` での worktree 存在確認

- **Context**: reopen 時に既存 worktree があればスキップし、コメントメッセージを切り替える
- **Alternatives Considered**:
  1. `worktree.create()` がエラーを返した場合に既存として扱う（try/catch パターン）
  2. `Path::exists()` で事前確認してから分岐
- **Selected Approach**: `Path::new(&wt_path).exists()` で事前確認
- **Rationale**: エラーを正常系として扱うのは設計として不明確。事前確認で意図が明示的になる
- **Trade-offs**: ファイルシステムの TOCTOU（time-of-check-time-of-use）問題が理論上あるが、worktree 作成はシングルスレッドのポーリングループ内で実行されるため実用上問題なし
- **Follow-up**: 既存 worktree の場合、`issue.worktree_path` が DB に既にセットされているため `issue_repo.update()` をスキップできる（または冪等なので呼んでも問題ない）

## Risks & Mitigations

- **PR チェックの GitHub API エラー** → フェイルセーフとして API エラー時は通常のセッション起動にフォールバック（要件 4.6）
- **worktree が部分的に存在する場合**（main ブランチは存在するが design ブランチがない等）→ `create_branch` や `push` は冪等な実装を前提とするが、エラーが発生した場合は `initialize_issue` が失敗してリトライポリシーが働く
- **cleanup コマンド実行中に polling loop が動作している場合** → cleanup は独立したプロセスとして実行されるが、daemon が動いている場合にリソース競合が起きる可能性がある。ドキュメントで daemon 停止後に実行することを推奨する

## References

- [Cupola ステートマシン](../../steering/product.md) — 10 状態のステートマシン定義
- [Clean Architecture 構造](../../steering/structure.md) — レイヤー構成・命名規約
- [技術スタック](../../steering/tech.md) — Rust + tokio + SQLite + octocrab の技術詳細
