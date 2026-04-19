# 要件定義

## はじめに

子プロセス（Claude Code）の stdout/stderr を **実行中にファイルへストリーム書き込み** する設計に切り替える。現状は `read_to_string` による in-memory 蓄積のため、30 分稼働の Claude Code セッションで stderr が肥大化し OOM に至るリスクがある。本要件は、ストリーム書き込みへの移行によって OOM 防止・デバッグ性向上・設計整合・コード単純化の 4 点を同時に達成することを目的とする。

## 要件

### Requirement 1: ストリーム書き込みによる OOM 防止

**目的:** インフラ運用者として、子プロセスの stdout/stderr がメモリ上に無制限蓄積されない設計を望む。これにより OOM Kill によるデーモン停止を防ぐ。

#### 受け入れ基準

1. When プロセスが `SessionManager.register` に登録されたとき、`SessionManager` は stdout および stderr を `.cupola/logs/process-runs/` 配下のファイルへストリーム書き込みするリーダースレッドを起動しなければならない。
2. While プロセスが実行中である間、`SessionManager` はメモリ上に stdout/stderr の文字列バッファを保持してはならない。
3. The `SessionManager` shall `JoinHandle<String>` 型の `stdout_handle` / `stderr_handle` フィールドを `SessionEntry` から削除し、代わりにファイルパス (`PathBuf`) を保持しなければならない。
4. The `SessionManager` shall stdout/stderr ファイルへのリーダースレッドにおいて `std::io::copy` を使用し、`BufWriter` 経由でバッファリングしてファイルへ書き込まなければならない。
5. The `SessionManager` shall `register(..., run_id)` 呼び出し時に確定した `run_id` を用いて `.cupola/logs/process-runs/run-{id}-stdout.log` および `.cupola/logs/process-runs/run-{id}-stderr.log` を決定し、一時ファイルや `update_run_id` によるリネームを前提とせずに書き込みを開始しなければならない。

### Requirement 2: デバッグ目的のリアルタイムログ可視化

**目的:** 開発者として、実行中のプロセス出力を `tail -f .cupola/logs/process-runs/run-{id}-stdout.log` で観察できるようにしたい。これにより、Claude Code の動作をリアルタイムでデバッグできる。

#### 受け入れ基準

1. The `SessionManager` shall 書き込みの粒度を確保するため、`LineWriter` またはフラッシュ間隔の設定を通じて、プロセス出力が遅延なくファイルへ反映されるようにしなければならない。
2. While プロセスが実行中である間、`.cupola/logs/process-runs/run-{id}-stdout.log` および `.cupola/logs/process-runs/run-{id}-stderr.log` は外部プロセス（例: `tail -f`）から読み取り可能でなければならない。
3. When ログファイルが存在する場合、ファイルはプロセスが終了する前から書き込まれており、実行開始後すぐに内容が観察できなければならない。

### Requirement 3: プロセス完了後の stdout/stderr 読み取り

**目的:** システムとして、プロセス終了後に stdout から PR 作成出力を解析し、stderr からエラースニペットを取得できる必要がある。

#### 受け入れ基準

1. The `ExitedSession` shall `stdout: String` / `stderr: String` フィールドを削除し、代わりに `stdout_path: PathBuf` / `stderr_path: PathBuf` を保持しなければならない。
2. When resolve フェーズで `ExitedSession` の stdout を解析する場合、システムは `std::fs::read_to_string(&session.stdout_path)` を使用してファイルから読み取らなければならない。
3. When resolve フェーズで `ExitedSession` の stderr からエラースニペットを生成する場合、システムはファイルから読み取った内容を使用しなければならない。
4. The `resolve.rs` shall `dump_session_io` 関数を削除しなければならない（ストリーム側で書き込みが完了しているため）。

### Requirement 4: ファイル書き込み失敗時のフォールバック

**目的:** 運用者として、ファイル書き込みが失敗した場合に、空の内容で PR が誤って作成されないことを保証したい。

#### 受け入れ基準

1. If リーダースレッド内でファイル作成（`File::create`）が失敗した場合、`SessionManager` は `tracing::error!` を出力してスレッドを終了させなければならない（子プロセス自体は継続する）。
2. If resolve フェーズでファイルが存在しない、またはファイルの読み取りに失敗した場合、システムは `mark_failed(run_id, "stdout log unavailable: ...")` を呼び出して PR 作成を中断しなければならない。
3. The システム shall ファイル読み取り失敗時に空文字列でフォールバックして PR 作成を試みてはならない。
4. When ファイル書き込みに失敗した場合、`tracing::error!` ログには対象ファイルパスとエラー詳細が含まれなければならない。

### Requirement 5: ログディレクトリとファイルパス管理

**目的:** システムとして、プロセスログファイルのパスを一貫したルールで管理する必要がある。

#### 受け入れ基準

1. The システム shall stdout ログを `.cupola/logs/process-runs/run-{run_id}-stdout.log`、stderr ログを `.cupola/logs/process-runs/run-{run_id}-stderr.log` として保存しなければならない。
2. The システム shall ログディレクトリ `.cupola/logs/process-runs/` を `register` 時に確実に作成しなければならない。

### Requirement 6: 既存テストの移行

**目的:** 開発者として、ファイルベースへの移行後も既存のテストカバレッジが維持されることを保証したい。

#### 受け入れ基準

1. The `session_manager.rs` のテストは `stdout` / `stderr` 文字列比較の代わりに、ファイルから読み取った内容を検証しなければならない。
2. The `resolve.rs` のテストにおける `make_session` ヘルパーは `stdout: String` / `stderr: String` の代わりに、テスト用ファイルへのパスを提供しなければならない。
3. When 統合テストを実行する場合、プロセス実行中に stdout/stderr ファイルが書き込まれることを検証しなければならない。
4. The 既存の resolve 系テスト（PR 作成・mark_failed・branch 検証）はすべて、ファイルパスベースの `ExitedSession` で動作しなければならない。
