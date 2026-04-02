# Requirements Document

## Project Description (Input)
cupola logs コマンドの追加（-f オプション付き）: cupola logs コマンドを追加し、ログファイルの内容を表示する。-f オプションで tail -f 相当のリアルタイム追跡を可能にする。cupola.toml の log.dir からログディレクトリを読み取る。

## Introduction

cupola はデーモンモード（`cupola start -d`）で起動した場合、ターミナルへのログ出力がなくなり、ログファイルのみに出力される。現状ではログを確認するためにログファイルを直接開く必要があり、運用上の煩雑さがある。

本機能では `cupola logs` コマンドを追加し、最新のログファイルの末尾を表示する機能と、`-f` オプションによるリアルタイムのログ追跡機能を提供する。これにより、デーモン稼働中のログ確認を CLI 上で完結できるようになる。

## Requirements

### Requirement 1: ログ表示コマンドの追加

**Objective:** As a cupola オペレーター, I want `cupola logs` コマンドでログファイルの末尾を確認できること, so that デーモン実行中のログを素早く確認できる。

#### Acceptance Criteria

1. When `cupola logs` コマンドが実行される, the cupola CLI shall `cupola.toml` の `[log] dir` 設定を読み込む。
2. When `cupola logs` コマンドが実行される, the cupola CLI shall `log.dir` ディレクトリ内で最新の日付のログファイルを特定し、その末尾を標準出力に表示する。
3. If `cupola.toml` に `log.dir` が設定されていない場合, the cupola CLI shall エラーメッセージ「`[log] dir is not configured in cupola.toml`」を標準エラー出力に表示し、非ゼロの終了コードで終了する。
4. If `log.dir` ディレクトリが存在しない場合, the cupola CLI shall エラーメッセージを標準エラー出力に表示し、非ゼロの終了コードで終了する。
5. If `log.dir` ディレクトリにログファイルが存在しない場合, the cupola CLI shall エラーメッセージ「`No log files found in <dir>`」を標準エラー出力に表示し、非ゼロの終了コードで終了する。
6. The cupola CLI shall `--config` オプション（デフォルト: `.cupola/cupola.toml`）でコンフィグファイルのパスを指定できる。

### Requirement 2: ログファイルのリアルタイム追跡（-f オプション）

**Objective:** As a cupola オペレーター, I want `cupola logs -f` でリアルタイムにログを追跡できること, so that デーモンの動作を監視しながら運用できる。

#### Acceptance Criteria

1. When `cupola logs -f` が実行される, the cupola CLI shall 最新のログファイルの末尾を表示した後、ファイルへの追記を継続的に監視し、新たに書き込まれた内容を随時標準出力に出力する。
2. While `-f` オプションでログ追跡中, the cupola CLI shall `Ctrl+C`（SIGINT）シグナルを受信した際にリアルタイム追跡を停止し、正常終了する。
3. When `cupola logs -f` が実行される, the cupola CLI shall `tail -f` 相当の動作（ファイル末尾のポーリングまたはinotify/kqueue等のOSネイティブ通知による監視）を行う。
4. The cupola CLI shall `-f` / `--follow` の両方の形式でフォローオプションを受け付ける。

### Requirement 3: ログファイルの選択

**Objective:** As a cupola オペレーター, I want 表示対象のログファイルが自動的に最新のものになること, so that 手動でファイル名を指定する必要がない。

#### Acceptance Criteria

1. The cupola CLI shall `log.dir` ディレクトリ内でファイル名のソート順が最後となるファイルを「最新」として選択する（`tracing-appender` が生成するファイル名形式 `cupola.YYYY-MM-DD` に対応）。
2. When 複数のログファイルが存在する場合, the cupola CLI shall 最新の1ファイルのみを対象とする。
3. The cupola CLI shall デフォルトで末尾20行を表示する。
4. Where `-n <N>` オプションが指定された場合, the cupola CLI shall 末尾 N 行を表示する。

### Requirement 4: アーキテクチャ適合

**Objective:** As a 開発者, I want `logs` コマンドがクリーンアーキテクチャに沿って実装されること, so that コードベースの一貫性が保たれる。

#### Acceptance Criteria

1. The cupola CLI shall `logs` サブコマンドを `src/adapter/inbound/cli.rs` の `Command` enum に追加する。
2. The cupola CLI shall ログファイルの読み取りとフォロー機能をユースケースとして `src/application/` 層に実装し、アダプター層から呼び出す。
3. The cupola CLI shall ファイルシステムアクセスの抽象化（ポート定義）を `src/application/port/` に定義し、テスト可能性を確保する。
4. The cupola CLI shall `bootstrap/app.rs` の `run()` 関数に `Command::Logs` ブランチを追加し、依存性注入を行う。
