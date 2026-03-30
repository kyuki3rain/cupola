# Requirements Document

## Project Description (Input)
cupola init コマンドの拡張: SQLiteスキーマ初期化に加え、cupola.toml雛形生成、steeringテンプレートコピー、.gitignore追記を自動化する。既存ファイルは上書きしない冪等設計。

## Introduction
`cupola init` コマンドを拡張し、プロジェクト初回セットアップに必要なファイル群（cupola.toml、steering テンプレート、.gitignore エントリ）を自動生成する。既存の SQLite スキーマ初期化機能は維持しつつ、冪等な設計で安全に繰り返し実行可能にする。

## Requirements

### Requirement 1: SQLite スキーマ初期化（既存機能維持）
**Objective:** 開発者として、既存の SQLite スキーマ初期化機能が引き続き動作することを保証したい。拡張によって既存機能が壊れないようにするため。

#### Acceptance Criteria
1. When `cupola init` コマンドが実行された場合, the cupola CLI shall create a SQLite データベースファイル and initialize the スキーマ
2. When the SQLite データベースが既に初期化済みの場合, the cupola CLI shall complete processing successfully without raising an error

### Requirement 2: cupola.toml 雛形生成
**Objective:** 開発者として、`cupola init` 実行時に cupola.toml の雛形が自動生成されることで、手動でファイルを作成する手間を省きたい。

#### Acceptance Criteria
1. When `cupola init` コマンドが実行された場合 and `.cupola/cupola.toml` が存在しない場合, the cupola CLI shall owner・repo・default_branch が空欄のテンプレート cupola.toml を `.cupola/cupola.toml` に生成する
2. While `.cupola/cupola.toml` が既に存在する場合, the cupola CLI shall 既存ファイルを上書きせずスキップする

### Requirement 3: steering テンプレートコピー
**Objective:** 開発者として、`cupola init` 実行時に steering テンプレートが自動配置されることで、cc-sdd のセットアップを簡略化したい。

#### Acceptance Criteria
1. When `cupola init` コマンドが実行された場合 and `.cupola/steering/` ディレクトリが空の場合, the cupola CLI shall `.cupola/settings/templates/steering/` 配下のテンプレートファイルを `.cupola/steering/` にコピーする
2. While `.cupola/steering/` ディレクトリにファイルが既に存在する場合, the cupola CLI shall テンプレートコピーをスキップする
3. If `.cupola/settings/templates/steering/` ディレクトリが存在しない場合（cc-sdd 未インストール）, the cupola CLI shall テンプレートコピーをスキップし、その旨をログに出力する

### Requirement 4: .gitignore 追記
**Objective:** 開発者として、`cupola init` 実行時に .gitignore に cupola 用エントリが自動追記されることで、不要ファイルの誤コミットを防ぎたい。

#### Acceptance Criteria
1. When `cupola init` コマンドが実行された場合 and `.gitignore` に cupola 用エントリが未追加の場合, the cupola CLI shall cupola 用の gitignore エントリを `.gitignore` に追記する
2. While `.gitignore` に cupola 用エントリが既に存在する場合, the cupola CLI shall 重複追記せずスキップする
3. If `.gitignore` ファイルが存在しない場合, the cupola CLI shall `.gitignore` ファイルを新規作成し、cupola 用エントリを書き込む

### Requirement 5: 冪等性
**Objective:** 開発者として、`cupola init` を何度実行しても同じ結果になることを保証したい。誤って既存設定を破壊するリスクを排除するため。

#### Acceptance Criteria
1. The cupola CLI shall `cupola init` を複数回実行しても、既存ファイルの内容を上書き・削除するなどの破壊的変更を行わない（ただし Requirement 4 で定義された `.gitignore` への初回の cupola 用エントリ追記は許可される）
2. When `cupola init` が2回連続で実行された場合, the cupola CLI shall 2回目の実行で全てのファイル生成ステップをスキップし、正常終了する

### Requirement 6: スコープ外の明示
**Objective:** 開発者として、このフィーチャーのスコープ外事項を明確にし、将来の混乱を防ぎたい。

#### Acceptance Criteria
1. The cupola CLI shall 対話的な入力プロンプトを表示しない
2. The cupola CLI shall cc-sdd のインストール処理を行わない
3. The cupola CLI shall GitHub ラベルの自動作成を行わない
