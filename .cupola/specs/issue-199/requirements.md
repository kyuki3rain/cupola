# 要件定義書

## プロジェクト概要 (入力)

`cupola init` コマンドに `--upgrade` フラグを実装する。現在のドキュメント (`docs/commands/init.md`) にはこのフラグの動作が記述されているが、実装が存在しない。`--upgrade` 実行時はバイナリに同梱された最新のCupola管理ファイル（ルール・テンプレート・スキル）を既存ファイルに上書きし、ユーザー所有ファイル（`cupola.toml`・ステアリング・スペック）は保護する。

## 要件

### 要件 1: `--upgrade` CLIフラグの追加

**目的:** Cupolaユーザーとして、`cupola init --upgrade` を実行することで、ユーザー設定を失わずにCupola管理ファイルを最新版に更新したい。これにより、Cupolaのバージョンアップ後も最新のガイダンスファイルを利用できる。

#### 受け入れ基準

1. When `cupola init --upgrade` を実行した場合、the init system shall `--upgrade` フラグを認識し、アップグレードモードで初期化処理を実行する。
2. When `cupola init` をフラグなしで実行した場合、the init system shall 従来どおり既存ファイルをスキップする動作を維持する。
3. The init system shall `--upgrade` フラグのデフォルト値を `false` とする。
4. When `--upgrade` と `--agent` フラグを同時に指定した場合、the init system shall 両フラグを独立して処理する。

### 要件 2: Cupola管理ファイルの上書き

**目的:** Cupolaユーザーとして、`--upgrade` 実行時にルール・テンプレート・スキルが最新バイナリ版で更新されることを保証したい。これにより、古い設定ファイルが残存するリスクを排除できる。

#### 受け入れ基準

1. While `upgrade=true` の場合、the init system shall `.cupola/settings/rules/` 配下の全ファイルをバイナリ同梱の最新版で上書きする。
2. While `upgrade=true` の場合、the init system shall `.cupola/settings/templates/` 配下の全ファイルをバイナリ同梱の最新版で上書きする。
3. While `upgrade=true` の場合、the init system shall `.claude/commands/cupola/` 配下の全スキルファイルをバイナリ同梱の最新版で上書きする。
4. While `upgrade=false` の場合、the init system shall 既存ファイルが存在する場合はスキップし、ファイルを上書きしない。

### 要件 3: ユーザー所有ファイルの保護

**目的:** Cupolaユーザーとして、`--upgrade` 実行時でもカスタム設定・ステアリング・スペックが変更されないことを保証したい。これにより、安心してアップグレードを実行できる。

#### 受け入れ基準

1. While `upgrade=true` の場合でも、the init system shall `.cupola/cupola.toml` を変更しない。
2. While `upgrade=true` の場合でも、the init system shall `.cupola/steering/` 配下のファイルを変更しない。
3. While `upgrade=true` の場合でも、the init system shall `.cupola/specs/` 配下のファイルを変更しない。
4. The init system shall `--upgrade` 実行時と非実行時で同一の保護対象ファイル分類を適用する。

### 要件 4: `.gitignore` エントリのアップグレード

**目的:** Cupolaユーザーとして、`--upgrade` 実行時に `.gitignore` のCupola管理セクションが最新版に更新されることを期待する。これにより、新しいエントリが追加されても自動的に反映される。

#### 受け入れ基準

1. While `upgrade=true` かつ `.gitignore` にCupolaマーカーが存在する場合、the init system shall 既存のCupola管理セクションを最新のエントリセットで置換する。
2. While `upgrade=true` かつ `.gitignore` にCupolaマーカーが存在しない場合、the init system shall 通常通りエントリを末尾に追記する。
3. While `upgrade=false` の場合、the init system shall Cupolaマーカーが既に存在する場合はスキップする（既存動作の維持）。
4. When `.gitignore` のCupola管理セクションを置換する場合、the init system shall セクション外のユーザー定義エントリを変更しない。

### 要件 5: アップグレード処理結果の報告

**目的:** Cupolaユーザーとして、アップグレード実行後に何が更新されたかを確認したい。これにより、処理結果を把握し問題を検出できる。

#### 受け入れ基準

1. When `--upgrade` を実行した場合、the init system shall 上書きされたファイルの種別（エージェントアセット・gitignore）を出力する。
2. When `--upgrade` を実行したが更新するファイルが存在しなかった場合、the init system shall その旨をユーザーに通知する（スキップではなく最新状態を意味する）。
3. The init system shall アップグレード処理中のエラーを適切なエラーメッセージとともに報告し、処理を中断する。
