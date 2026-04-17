# 要件定義書

## はじめに

本仕様は Issue #194 に対応するバグ修正の要件を定義する。

`CloseIssue` および `CleanupWorktree` エフェクトハンドラにおいて、GitHub API / Git コマンドの実行が成功した後に DB 更新 (`update_state_and_metadata`) が失敗した場合、インメモリの `issue` 状態が更新されないため、次のポーリングサイクルでエフェクトが再試行されず、ステートマシンが不整合状態に陥る問題を修正する。

具体的には以下の 2 つのシナリオが対象:

- **CloseIssue**: `close_finished=false` のままになり、`Cancelled→Idle` 遷移が永遠にブロックされる
- **CleanupWorktree**: `worktree_path` が非 null のままになり、意図しないリトライループが発生する（冪等性により安全だが望ましくない）

推奨修正方針: Option B + モニタリング — 高影響エフェクトの DB 更新呼び出しを指数バックオフ付きリトライでラップし、部分的失敗を明示的にログ出力する。

## 要件

### Requirement 1: エフェクト後 DB 更新のリトライ機構

**Objective:** システム運用者として、GitHub API / Git コマンドの成功後に DB 更新が一時的に失敗しても自動的にリカバリされるようにしたい。そうすることで、ステートマシンが不整合状態に陥ることなく正常に遷移できる。

#### Acceptance Criteria

1. When `CloseIssue` エフェクト実行中に GitHub `close_issue` API 呼び出しが成功した後で `update_state_and_metadata` が失敗した場合, the ポーリングエグゼキュータ shall 最大 3 回（遅延: 1 秒、2 秒、4 秒）の指数バックオフリトライで DB 更新を再試行する
2. When `CleanupWorktree` エフェクト実行中にworktreeの削除が成功した後で `update_state_and_metadata` が失敗した場合, the ポーリングエグゼキュータ shall 最大 3 回（遅延: 1 秒、2 秒、4 秒）の指数バックオフリトライで DB 更新を再試行する
3. If DB 更新が全リトライ回数を使い切った場合, the ポーリングエグゼキュータ shall エラーを返してインメモリ状態を変更せず、次のポーリングサイクルでエフェクト全体が再試行されるようにする
4. The ポーリングエグゼキュータ shall リトライロジックを汎用ヘルパーとして実装し、`CloseIssue` と `CleanupWorktree` の両エフェクトから再利用できるようにする
5. While DB 更新リトライ中, the ポーリングエグゼキュータ shall ポーリングループ全体をブロックせずにリトライを実行する（非同期スリープを使用する）

### Requirement 2: 部分的失敗の観測可能性

**Objective:** システム運用者として、API 成功 + DB 更新失敗という部分的失敗パターンを即座に把握できるようにしたい。そうすることで、永続的な障害が発生した際に迅速に対処できる。

#### Acceptance Criteria

1. When DB 更新が 1 回以上失敗してリトライが発生した場合, the ポーリングエグゼキュータ shall `warn` レベルで失敗したエフェクト種別・Issue 番号・試行回数・エラー内容をログ出力する
2. If DB 更新が全リトライ回数を使い切って永続的に失敗した場合, the ポーリングエグゼキュータ shall `error` レベルで「API は成功したが DB 更新が永続的に失敗した」ことを示すログを出力する
3. The ポーリングエグゼキュータ shall ログに Issue 番号とエフェクト種別（`CloseIssue` / `CleanupWorktree`）を構造化フィールドとして含める

### Requirement 3: テストによる動作保証

**Objective:** 開発者として、リトライ機構が期待通りに動作することをテストで検証したい。そうすることで、将来の変更によるリグレッションを防止できる。

#### Acceptance Criteria

1. When `update_state_and_metadata` が 1 回失敗した後に成功するようにモックされている場合, the `CloseIssue` ユニットテスト shall 2 回目の試行で `close_finished=true` がセットされることを検証する
2. When `update_state_and_metadata` が永続的に失敗するようにモックされている場合, the `CleanupWorktree` ユニットテスト shall エフェクト実行がエラーを返しインメモリ状態が変更されないことを検証する
3. The リトライヘルパーユニットテスト shall リトライ回数・バックオフ間隔・最終エラー伝播の各ロジックを個別に検証する
4. If リトライが全回数を消費した場合, the ポーリングエグゼキュータ shall インメモリの `issue.close_finished` および `issue.worktree_path` が変更されないことをテストで確認できる
