# Requirements Document

## Introduction
Cupola の OSS 公開に向けて README.md を作成する。初めてリポジトリを訪れた開発者が、プロジェクトの目的を理解し、セットアップから `cupola run` の実行まで再現可能な手順で到達できるドキュメントを整備する。対象読者は GitHub Issue / PR ベースの開発ワークフローに慣れたエンジニアである。

## Requirements

### Requirement 1: プロジェクト概要セクション
**Objective:** As a 初めてリポジトリを訪れた開発者, I want プロジェクトの目的と価値提案を素早く把握したい, so that 自分のユースケースに合うか判断できる

#### Acceptance Criteria
1. The README shall プロジェクト概要を 3 文以内で記述する
2. The README shall Cupola が「GitHub Issue を起点に設計・実装を自動化するローカル常駐エージェント」であることを明記する
3. The README shall 人間の役割（Issue 作成・ラベル付与・PR レビュー）と自動化される範囲（設計ドキュメント生成〜実装〜レビュー対応）を区別して説明する

### Requirement 2: 前提条件セクション
**Objective:** As a Cupola を導入したい開発者, I want 必要なツールとバージョンの一覧を確認したい, so that 環境構築前に準備すべきものを把握できる

#### Acceptance Criteria
1. The README shall 以下の前提条件を一覧で記述する: Rust stable, Claude Code CLI, gh CLI, Git, devbox
2. The README shall cc-sdd（spec-driven development）の概要と Cupola との関係を簡潔に説明する
3. Where devbox が利用可能な場合, the README shall devbox による一括セットアップ手順を案内する

### Requirement 3: インストール・セットアップ手順セクション
**Objective:** As a Cupola を初めて使う開発者, I want ゼロから動作可能な状態まで再現可能な手順を知りたい, so that 迷わずセットアップを完了できる

#### Acceptance Criteria
1. The README shall `cargo build` によるビルド手順を記述する
2. The README shall `cupola.toml` の設定項目と記述例を提供する
3. The README shall `cupola init` による SQLite スキーマ初期化手順を記述する
4. The README shall GitHub 上での `agent:ready` ラベル作成手順を記述する
5. When 手順に従って実行した場合, the README shall `cupola run` が正常に polling を開始できる状態に至る手順であること

### Requirement 4: 使い方セクション
**Objective:** As a Cupola をセットアップ済みの開発者, I want 日常の使い方（Issue → 設計 → 実装 → マージの流れ）を理解したい, so that 開発ワークフローに組み込める

#### Acceptance Criteria
1. The README shall Issue 作成から merge までの一連のワークフローをステップバイステップで記述する
2. The README shall 各ステップにおける人間の操作と Cupola の自動処理を明確に区別する
3. The README shall `agent:ready` ラベル付与が Cupola のトリガーとなることを説明する
4. The README shall 設計 PR と実装 PR の 2 段階レビューフローを説明する

### Requirement 5: CLI コマンドリファレンスセクション
**Objective:** As a Cupola ユーザー, I want 全 CLI コマンドの用途とオプションを参照したい, so that 必要な操作を正確に実行できる

#### Acceptance Criteria
1. The README shall `run` サブコマンド（polling ループ開始）の説明を記述する
2. The README shall `init` サブコマンド（SQLite スキーマ初期化）の説明を記述する
3. The README shall `status` サブコマンド（Issue 状態一覧表示）の説明を記述する
4. The README shall 各コマンドの実行例をコードブロックで提供する

### Requirement 6: 設定ファイルリファレンスセクション
**Objective:** As a Cupola ユーザー, I want `cupola.toml` の全設定項目と意味を参照したい, so that プロジェクトに合わせた設定ができる

#### Acceptance Criteria
1. The README shall `cupola.toml` の全設定項目を網羅する
2. The README shall 各項目の型、デフォルト値、説明を記述する
3. The README shall 設定ファイルの完全な記述例を提供する

### Requirement 7: アーキテクチャ概要セクション
**Objective:** As a Cupola にコントリビュートしたい開発者, I want コードベースの構造を理解したい, so that 変更すべき箇所を素早く特定できる

#### Acceptance Criteria
1. The README shall Clean Architecture 4 レイヤー（domain / application / adapter / bootstrap）の構成を説明する
2. The README shall 各レイヤーの責務と依存方向を簡潔に説明する
3. The README shall `src/` 配下のディレクトリ構造を示す

### Requirement 8: ライセンス情報セクション
**Objective:** As a 利用・コントリビュートを検討する開発者, I want ライセンスを確認したい, so that 利用条件を把握できる

#### Acceptance Criteria
1. The README shall ライセンス情報を記述する
2. If リポジトリに LICENSE ファイルが存在する場合, the README shall そのファイルへのリンクを含める
