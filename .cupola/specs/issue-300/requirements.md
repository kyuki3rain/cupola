# Requirements Document

## Project Description (Input)
## 背景

現在 spec ディレクトリの初期化（`spec.json` + `requirements.md` 雛形生成）は cupola daemon の Rust コード（SpawnInit 内）でのみ実行できる。手動で Claude Code から spec-driven workflow を使う場合、初期化手段がない。

## やること

`/cupola:spec-init {spec-id}` スキルを追加する。

- `.cupola/specs/{spec-id}/` ディレクトリ作成
- `spec.json` の生成（言語設定等）
- `requirements.md` 雛形の生成

## 設計方針

- cupola daemon は引き続き Rust で直接 spec-init を実行する（速度・確実性のため）
- スキルは手動ワークフロー向けに同等の機能を提供する
- 既存の Rust 側の初期化処理と生成されるファイルの形式を一致させる

## 手動ワークフロー

```
/cupola:spec-init issue-123    ← spec ディレクトリ初期化
/cupola:spec-design issue-123  ← 要件・設計・タスク生成
/cupola:spec-impl issue-123    ← TDD 実装
```

## 関連

- #298 (スキルの自立化と cupola のオーケストレーター化)
- #299 (fixing プロンプトのスキル統一)

## Requirements

### Requirement 1: スキルの呼び出しインターフェース

**Objective:** 手動ワークフローを実行する開発者として、`/cupola:spec-init` スキルで spec ディレクトリを初期化したい。それにより Rust デーモンなしで spec-driven workflow を開始できる。

#### Acceptance Criteria

1. When `/cupola:spec-init {spec-id}` が実行される, the spec-init skill shall `spec-id` に対応する spec ディレクトリを作成する。
2. If `spec-id` 引数が未指定の場合, the spec-init skill shall 使用方法を表示して処理を停止する。
3. The spec-init skill shall オプションの言語引数（例: `ja`, `en`）を受け付ける。
4. When 言語引数が指定されない場合, the spec-init skill shall `.cupola/cupola.toml` から言語設定を読み込み、存在しない場合は `ja` をデフォルト値として使用する。
5. When spec の初期化が完了する, the spec-init skill shall 次のワークフローステップ（`/cupola:spec-design {spec-id}`）の案内を表示する。

### Requirement 2: spec ディレクトリとファイルの生成

**Objective:** 開発者として、spec-init スキルが正しいディレクトリ構造と初期ファイルを生成してほしい。それにより後続の spec-design ワークフローが正しい入力を受け取れる。

#### Acceptance Criteria

1. When spec-init が spec-id を受け取り実行される, the spec-init skill shall `.cupola/specs/{spec-id}/` ディレクトリを作成する。
2. When spec-init が実行される, the spec-init skill shall spec ディレクトリ内に `spec.json` を生成する。
3. When spec-init が実行される, the spec-init skill shall spec ディレクトリ内に `requirements.md` を生成する。
4. The spec-init skill shall `.cupola/settings/templates/specs/init.json` テンプレートを使用して `spec.json` を生成する。
5. The spec-init skill shall `.cupola/settings/templates/specs/requirements-init.md` テンプレートを使用して `requirements.md` を生成する。

### Requirement 3: 生成ファイルの内容

**Objective:** 手動ワークフローの開発者として、生成されるファイルが Rust デーモンの出力形式と一致してほしい。それにより spec-design、spec-impl などの後続ツールが正しく動作する。

#### Acceptance Criteria

1. The spec-init skill shall 生成する `spec.json` の `feature_name` に指定された spec-id を設定する。
2. The spec-init skill shall 生成する `spec.json` の `created_at` と `updated_at` に ISO 8601 形式の現在 UTC タイムスタンプを設定する。
3. The spec-init skill shall 生成する `spec.json` の `language` に解決済みの言語値を設定する。
4. The spec-init skill shall 生成する `spec.json` の `phase` に `"initialized"` を設定する。
5. The spec-init skill shall 生成する `spec.json` の全 approval フィールド（`requirements`, `design`, `tasks`）に `{"generated": false, "approved": false}` を設定する。
6. The spec-init skill shall 生成する `spec.json` の `ready_for_implementation` に `false` を設定する。
7. The spec-init skill shall 生成する `requirements.md` にプロジェクト説明の記入箇所を含める。

### Requirement 4: 冪等性と安全性

**Objective:** 開発者として、spec-init が既存データを誤って上書きしないことを保証してほしい。それにより既存の spec 作業を安全に保てる。

#### Acceptance Criteria

1. If spec ディレクトリが既に存在する場合, the spec-init skill shall ファイルを上書きせずに既存ディレクトリの存在を報告して停止する。
2. If `.cupola/` ディレクトリが存在しない（プロジェクト未初期化）場合, the spec-init skill shall エラーを報告して `cupola init` の実行を促す。
3. If spec-id に無効な文字（英数字・ハイフン・アンダースコア以外）が含まれる場合, the spec-init skill shall バリデーションエラーを報告して停止する。

### Requirement 5: cupola init によるスキルのインストール

**Objective:** cupola を使用するプロジェクトの開発者として、`cupola init` 実行後に `/cupola:spec-init` スキルが利用可能になってほしい。それにより手動ワークフローをすぐに開始できる。

#### Acceptance Criteria

1. When `cupola init` が実行される, the cupola CLI shall `.claude/commands/cupola/spec-init.md` スキルファイルをプロジェクトにインストールする。
2. When `cupola init --upgrade` が実行される, the cupola CLI shall 既存の `spec-init.md` スキルファイルを最新バージョンに上書きする。
3. The spec-init.md skill file shall `CLAUDE_CODE_ASSETS` に登録され `include_str!()` でバイナリに埋め込まれる。
