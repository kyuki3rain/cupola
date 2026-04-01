# Requirements Document

## Project Description (Input)
cupola logs コマンドの追加（-f オプション付き）: ログファイルの内容を表示し、-f オプションで tail -f 相当のリアルタイム追跡を可能にする

## イントロダクション

`cupola logs` コマンドを追加し、デーモン起動（`cupola start -d`）時のログファイルを簡単に確認できるようにする。
`-f` オプションを指定することで `tail -f` 相当のリアルタイム追跡が可能となり、運用性を向上させる。

## Requirements

### Requirement 1: ログ末尾の表示

**Objective:** cupola オペレーターとして、`cupola logs` コマンドでログファイルの末尾を表示したい。デーモンの動作状況を素早く確認するため。

#### Acceptance Criteria

1. When `cupola logs` コマンドが実行されると、the Cupola CLI shall `cupola.toml` の `log.dir` で指定されたディレクトリから最新のログファイルを特定し、その末尾（デフォルト20行）を標準出力に表示する
2. When 複数のログファイルが存在する場合、the Cupola CLI shall ファイル名の日付順で最新のファイルを選択する
3. The Cupola CLI shall ログファイルの内容を変換・加工せず、そのままの形式で出力する

---

### Requirement 2: リアルタイムログ追跡

**Objective:** cupola オペレーターとして、`cupola logs -f` でログをリアルタイムに追跡したい。デーモンの動作をライブで監視するため。

#### Acceptance Criteria

1. When `cupola logs -f` コマンドが実行されると、the Cupola CLI shall 最新のログファイルを継続的に監視し、追記された内容をリアルタイムに標準出力へ出力する
2. While `-f` オプションで追跡中に、the Cupola CLI shall ファイルへの新規追記を検知するたびに即座に出力する
3. When Ctrl+C シグナル（SIGINT）を受信すると、the Cupola CLI shall リアルタイム追跡を安全に終了し、プロセスを正常終了する

---

### Requirement 3: ログディレクトリ設定の読み取り

**Objective:** cupola オペレーターとして、ログディレクトリの設定を `cupola.toml` から自動的に読み取ってほしい。個別に指定する手間を省くため。

#### Acceptance Criteria

1. The Cupola CLI shall `cupola logs` 実行時に `cupola.toml` の `log.dir` フィールドを参照してログディレクトリを決定する
2. If `cupola.toml` に `log.dir` が設定されていない場合、the Cupola CLI shall エラーメッセージ「log.dir が cupola.toml に設定されていません」を標準エラー出力に表示し、非ゼロの終了コードで終了する
3. If `log.dir` で指定されたディレクトリが存在しない場合、the Cupola CLI shall エラーメッセージを標準エラー出力に表示し、非ゼロの終了コードで終了する
4. If `log.dir` にログファイルが存在しない場合、the Cupola CLI shall エラーメッセージ「ログファイルが見つかりません」を標準エラー出力に表示し、非ゼロの終了コードで終了する

---

### Requirement 4: CLI インターフェース

**Objective:** cupola オペレーターとして、既存の cupola サブコマンド体系と一貫した使い方で `logs` コマンドを利用したい。

#### Acceptance Criteria

1. The Cupola CLI shall `logs` を新しいサブコマンドとして clap に登録し、`cupola logs` の形式で呼び出せるようにする
2. The Cupola CLI shall `-f` / `--follow` フラグをオプションとしてサポートする
3. When `cupola logs --help` が実行されると、the Cupola CLI shall コマンドの説明と利用可能なオプションを表示する
