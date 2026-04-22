# 要件定義書

## はじめに

`src/application/polling/execute.rs`（約2,870行）を Effect 種別ごとの executor モジュールに分割し、`ExecuteContext` generic struct を導入する機械的リファクタリング。ロジック変更は一切なく、コードの保守性・拡張性の向上が目的である。

`pub async fn execute_effects(...)` のシグネチャは完全に維持され、呼び出し元（`polling_use_case.rs`、統合テスト）への影響はゼロ。将来の #338（EffectLog port 追加）に備えた前提構造として `ExecuteContext` を整備する。

## Requirements

### Requirement 1: モジュール構成への移行

**Objective:** 開発者として、`execute.rs` を Effect 種別ごとのモジュールに分割したい。そうすることで、各 Effect の実装を独立したファイルで管理でき、コードの見通しと保守性が向上する。

#### Acceptance Criteria

1.1. When リファクタリングが完了したとき、the execute module shall `src/application/polling/execute/` ディレクトリ構成（`mod.rs`、`context.rs`、`dispatcher.rs`、`comment_executor.rs`、`spawn_init_executor.rs`、`spawn_process_executor.rs`、`worktree_executor.rs`、`close_executor.rs`、`shared.rs`）に移行している。

1.2. The execute module shall 各ファイルを 500 行以下に収める。

1.3. The execute module shall `retry_db_update`、`BodyTamperedError`、`sha256_hex`、`SpawnableGitWorktree` を `mod.rs` に配置する。

1.4. The execute module shall 共通ヘルパー関数（`get_pr_number_for_type`、`state_from_phase`、`phase_for_type`、`find_last_error`）を `shared.rs` に配置する。

1.5. The execute module shall 各 executor ファイルの可視性を `pub(super)` に制限し、`mod.rs` からのみアクセス可能にする。

### Requirement 2: ExecuteContext の導入

**Objective:** 開発者として、全実行引数を束ねる `ExecuteContext` struct を導入したい。そうすることで `#[allow(clippy::too_many_arguments)]` を除去し、将来の依存追加を Context フィールド追加だけで完了できる構造にする。

#### Acceptance Criteria

2.1. When `ExecuteContext` が定義されたとき、the context module shall `pub struct ExecuteContext<'a, G, I, P, C, W, F>` として 9 フィールド（`github`、`issue_repo`、`process_repo`、`claude_runner`、`worktree`、`file_gen`、`session_mgr`、`init_mgr`、`config`）を持つ generic struct を提供する。

2.2. The execute module shall `execute_effects` の内部で `ExecuteContext` を構築し、`dispatcher::dispatch` に渡す。

2.3. When executor 関数が実装されたとき、the execute module shall 全 executor 関数のシグネチャを `ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>` ベースに統一する。

2.4. The execute module shall `#[allow(clippy::too_many_arguments)]` アトリビュートを全ファイルから除去する。

### Requirement 3: Dispatcher の整備

**Objective:** 開発者として、`execute_one` の match 処理を `dispatcher.rs` に分離したい。そうすることで、Effect 種別の dispatch ロジックが一箇所に集約され、新 Effect 追加時の変更箇所が明確になる。

#### Acceptance Criteria

3.1. When Effect が dispatch されるとき、the dispatcher shall Effect enum の全バリアント（`PostCompletedComment`、`PostCancelComment`、`PostRetryExhaustedComment`、`RejectUntrustedReadyIssue`、`PostCiFixLimitComment`、`SpawnInit`、`SpawnProcess`、`SwitchToImplBranch`、`CleanupWorktree`、`CloseIssue`）を対応する executor 関数に委譲する。

3.2. The dispatcher shall `dispatch` 関数のみを `pub(super)` として公開し、内部実装を隠蔽する。

3.3. If Effect バリアントが match で未処理のとき、the Rust compiler shall コンパイルエラーを発生させる（exhaustive match による静的保証）。

### Requirement 4: パブリック API の維持

**Objective:** 開発者として、`execute_effects` のシグネチャを変更しないリファクタを行いたい。そうすることで、呼び出し元（`polling_use_case.rs`、統合テスト）への影響をゼロに抑えられる。

#### Acceptance Criteria

4.1. The execute module shall `pub async fn execute_effects<G, I, P, C, W, F>(github, issue_repo, process_repo, claude_runner, worktree, file_gen, session_mgr, init_mgr, config, issue, effects)` のシグネチャを完全に維持する。

4.2. The execute module shall `pub trait SpawnableGitWorktree` の公開範囲と定義を維持する。

4.3. When `polling_use_case.rs` がコンパイルされるとき、the build shall `execute_effects` の呼び出しに変更を加えることなくビルドに成功する。

### Requirement 5: コード品質の確保

**Objective:** 開発者として、リファクタ後もコード品質基準を満たしたい。そうすることで、CI がグリーンを保ち、将来の開発者が安心してコードを変更できる。

#### Acceptance Criteria

5.1. When `cargo clippy -D warnings` が実行されたとき、the build shall 警告なしで通過する。

5.2. When `cargo fmt --check` が実行されたとき、the build shall フォーマット差分なしで通過する。

5.3. When `cargo test` が実行されたとき、the build shall 既存の全テストが通過する。

5.4. The execute module shall ロジックの変更を一切含まない（機械的リファクタ + Context 導入のみ）。
