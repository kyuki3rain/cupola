# Requirements Document

## Project Description (Input)
execution_log の started_at がプロセス終了時刻になっているバグ修正: step7_spawn_processes で record_start を呼び、log_id を SessionManager で保持し、step3_process_exit_check では record_finish のみ呼ぶように修正する

## Introduction

`PollingUseCase` は Claude Code プロセスの生成（step7）から終了検知（step3）までを管理し、各実行を `execution_log` テーブルに記録する。現在、`step3_process_exit_check` 内でプロセス**終了後**に `record_start` を呼んでいるため、`started_at ≈ finished_at` となり、実際のプロセス実行時間が記録されないバグが存在する。

本修正では、`record_start` の呼び出しをプロセス spawn 時（step7）へ移動し、返された `log_id` を `SessionManager` に保持することで、step3 では `record_finish` のみを呼べるようにする。ドメイン層・DB schema の変更は不要。

## Requirements

### Requirement 1: プロセス開始時の実行ログ記録

**Objective:** システム運用者として、Claude Code プロセスが実際に開始された時刻を `execution_log.started_at` に記録したい。これにより、プロセスの実際の実行時間（waiting 時間を除く）を正確に把握できる。

#### Acceptance Criteria

1. When `step7_spawn_processes` でプロセスの spawn に成功したとき, the PollingUseCase shall `exec_log_repo.record_start(issue_id, state)` を呼び出し `log_id` を取得する
2. When `record_start` の呼び出しに成功したとき, the PollingUseCase shall 取得した `log_id` を `SessionManager.register()` に渡してセッション情報と紐づけて保持する
3. If `record_start` の呼び出しが失敗したとき, the PollingUseCase shall `log_id = 0` をフォールバック値として使用し、プロセス自体の spawn は中断しない
4. The PollingUseCase shall `step3_process_exit_check` から `record_start` の呼び出しを削除する

### Requirement 2: プロセス終了時の実行ログ完了記録

**Objective:** システム運用者として、Claude Code プロセスが終了した際に正確な終了時刻・終了コード・出力を `execution_log` に記録したい。これにより、各実行の開始から終了までの全情報をトレースできる。

#### Acceptance Criteria

1. When `step3_process_exit_check` でプロセスの終了を検知したとき, the PollingUseCase shall `ExitedSession` に含まれる `log_id` を使って `exec_log_repo.record_finish()` を呼び出す
2. When プロセスが正常終了（exit code 0）したとき, the PollingUseCase shall `record_finish(log_id, exit_code, Some(&stdout), None)` を呼び出す
3. When プロセスが異常終了（exit code != 0）したとき, the PollingUseCase shall `record_finish(log_id, exit_code, None, Some(&stderr))` を呼び出す
4. The PollingUseCase shall `record_finish` を呼び出す際に `record_start` を呼び出さない（step3 からの `record_start` 呼び出しが削除されている）

### Requirement 3: SessionManager による log_id の保持

**Objective:** アプリケーション開発者として、`SessionEntry` が `log_id` を保持し、プロセス終了時に `ExitedSession` 経由で受け取れるようにしたい。これにより、spawn 時に取得した `log_id` を終了検知まで安全に受け渡せる。

#### Acceptance Criteria

1. The SessionManager shall `register(issue_id, child, log_id)` シグネチャ（または同等の方法）で `log_id: i64` を受け取り `SessionEntry` に保存する
2. When `collect_exited()` がプロセス終了を検出したとき, the SessionManager shall `ExitedSession` に `log_id: i64` フィールドを含めて返す
3. The SessionManager shall 既存の `register()` を呼び出している `step7_spawn_processes` 以外のコードが壊れないようにシグネチャ変更の影響範囲を最小化する
4. While `SessionEntry` が存在する間, the SessionManager shall `log_id` を不変の値として保持し変更しない

### Requirement 4: 実行時間の正確な記録の保証

**Objective:** システム運用者として、`execution_log.started_at` と `finished_at` の差分がプロセスの実際の実行時間を表すことを保証したい。これにより、Issue ごとの処理コスト分析やボトルネック検出が可能になる。

#### Acceptance Criteria

1. When プロセスが spawn されてから終了するまでの時間が存在するとき, the PollingUseCase shall `finished_at - started_at > 0` となる実行ログを記録する
2. The PollingUseCase shall `started_at` が `finished_at` 以前の時刻になることを常に保証する（`started_at <= finished_at`）
3. When 同一 Issue に対して複数回プロセスが実行されるとき, the PollingUseCase shall それぞれの spawn ごとに独立した `execution_log` レコードを作成する
