# Requirements Document

## Introduction
`cupola doctor` コマンドは、cupola の実行に必要な前提条件を自動的にチェックし、結果を視覚的に表示する診断コマンドである。初回セットアップ時やトラブルシューティング時に、環境が正しく構成されているかを素早く確認できるようにすることを目的とする。

## Requirements

### Requirement 1: CLI サブコマンド登録
**Objective:** 開発者として、`cupola doctor` というサブコマンドを実行できるようにしたい。既存の `run` / `init` / `status` と同様にアクセスできるようにするため。

#### Acceptance Criteria
1. When ユーザーが `cupola doctor` を実行した場合, the cupola CLI shall 全前提条件チェックを順番に実行し結果を表示する
2. When ユーザーが `cupola --help` を実行した場合, the cupola CLI shall `doctor` サブコマンドをヘルプ出力に含める

### Requirement 2: 外部ツールのチェック
**Objective:** 開発者として、必要な外部ツール（git, claude, gh）が利用可能かを自動チェックしたい。実行前に不足ツールを把握するため。

#### Acceptance Criteria
1. When `doctor` コマンドが実行された場合, the cupola CLI shall `git --version` を実行し Git が利用可能かをチェックする
2. When `doctor` コマンドが実行された場合, the cupola CLI shall `claude --version` を実行し Claude Code CLI が利用可能かをチェックする
3. When `doctor` コマンドが実行された場合, the cupola CLI shall `gh auth status` を実行し GitHub CLI が認証済みかをチェックする
4. If 外部ツールのコマンド実行が失敗した場合, the cupola CLI shall 該当チェックを失敗として記録する

### Requirement 3: agent:ready ラベルチェック
**Objective:** 開発者として、リポジトリに `agent:ready` ラベルが存在するかを確認したい。cupola のワークフローに必要なラベルが設定済みかを把握するため。

#### Acceptance Criteria
1. When `gh auth status` チェックが成功している場合, the cupola CLI shall `gh label list` を使用して `agent:ready` ラベルの存在を確認する
2. If `gh auth status` チェックが失敗している場合, the cupola CLI shall `agent:ready` ラベルチェックをスキップし、スキップした旨を表示する

### Requirement 4: 設定ファイルのチェック
**Objective:** 開発者として、cupola の設定ファイルが正しく構成されているかを確認したい。必須設定の不備を事前に検出するため。

#### Acceptance Criteria
1. When `doctor` コマンドが実行された場合, the cupola CLI shall `.cupola/cupola.toml` の存在を確認する
2. When `.cupola/cupola.toml` が存在する場合, the cupola CLI shall 必須項目（`owner`, `repo`, `default_branch`）が設定されているかを検証する
3. If `.cupola/cupola.toml` が存在しないか必須項目が欠落している場合, the cupola CLI shall 該当チェックを失敗として記録する

### Requirement 5: プロジェクト構成のチェック
**Objective:** 開発者として、steering ファイルと DB が適切にセットアップされているかを確認したい。`cupola init` が正常に完了しているかを検証するため。

#### Acceptance Criteria
1. When `doctor` コマンドが実行された場合, the cupola CLI shall `.cupola/steering/` ディレクトリにファイルが 1 つ以上存在するかを確認する
2. When `doctor` コマンドが実行された場合, the cupola CLI shall `.cupola/cupola.db` の存在を確認する（`cupola init` 済みかの確認）
3. If steering ディレクトリが空またはファイルが存在しない場合, the cupola CLI shall 該当チェックを失敗として記録する
4. If `.cupola/cupola.db` が存在しない場合, the cupola CLI shall 該当チェックを失敗として記録する

### Requirement 6: 結果表示
**Objective:** 開発者として、チェック結果を一目で把握できる形式で確認したい。問題の有無を素早く判断するため。

#### Acceptance Criteria
1. When 各チェック項目が成功した場合, the cupola CLI shall ✅ アイコンとチェック名を表示する
2. When 各チェック項目が失敗した場合, the cupola CLI shall ❌ アイコンとチェック名を表示する
3. When チェック項目が失敗した場合, the cupola CLI shall 具体的な修正手順を該当項目の直下に表示する

### Requirement 7: 終了コード
**Objective:** 開発者として、チェック結果をスクリプトから利用できるようにしたい。CI やセットアップスクリプトで自動判定するため。

#### Acceptance Criteria
1. When 全チェック項目が成功した場合, the cupola CLI shall 終了コード 0 で終了する
2. When 1 つ以上のチェック項目が失敗した場合, the cupola CLI shall 終了コード 1 で終了する
