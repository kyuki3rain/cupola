# Requirements Document

## はじめに

`cupola status` コマンドは、現在管理中の Issue ごとに番号・状態・ワークツリーパスを表示する。しかし、CI 修復の試行回数（`ci_fix_count`）や上限値（`max_ci_fix_cycles`）は画面に表示されず、運用中に「あと何回 CI 修復を試みられるか」を即座に把握できない。

本機能は、status 出力の各 Issue 行に `ci-fix: N` または `ci-fix: N/MAX` の表示を追加し、オペレーターが CI 修復状況を一目で確認できるようにする。

## Requirements

### Requirement 1: CI修復回数の常時表示

**Objective:** オペレーターとして、`cupola status` の各 Issue 行で CI 修復試行回数と上限を確認したい。それにより、打ち切り間近の Issue をすぐに特定できるようにする。

#### Acceptance Criteria

1. When `cupola status` を実行し `max_ci_fix_cycles` が設定されている場合, the status コマンド shall 各 Issue 行の末尾に `ci-fix: N/MAX` を表示する（N = `ci_fix_count`、MAX = `max_ci_fix_cycles`）
2. When `cupola status` を実行し `max_ci_fix_cycles` が未設定（設定ファイル不在 or 読み込み失敗）の場合, the status コマンド shall 各 Issue 行の末尾に `ci-fix: N` のみを表示する
3. The status コマンド shall `ci_fix_count` が 0 であっても `ci-fix: 0` または `ci-fix: 0/MAX` を表示する（常時表示）
4. When `ci_fix_count > max_ci_fix_cycles` かつ `ci_fix_limit_notified` が false の場合, the status コマンド shall `ci-fix: N/MAX` の後に ` ⚠ ci-fix-limit notification pending` を表示する（既存動作を維持）
5. The status コマンド shall `ci-fix` 情報を worktree パスの後に配置する

### Requirement 2: テスト整合性

**Objective:** 開発者として、フォーマット変更に対応したユニットテストを通じて出力の正確性を検証したい。

#### Acceptance Criteria

1. When `handle_status` の出力フォーマットが変更された場合, the テストスイート shall フォーマット変更に対応した更新済みアサーションを持つ
2. When `max_ci_fix_cycles` が設定されている場合の `handle_status` テスト, the テストスイート shall `ci-fix: N/MAX` 形式が出力されることを検証する
3. When `max_ci_fix_cycles` が未設定の場合の `handle_status` テスト, the テストスイート shall `ci-fix: N` 形式（スラッシュなし）が出力されることを検証する
4. When `ci_fix_count > max_ci_fix_cycles` かつ未通知の場合, the テストスイート shall `ci-fix: N/MAX ⚠ ci-fix-limit notification pending` が含まれることを検証する
