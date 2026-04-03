# Requirements Document

## Introduction

v0.1プレリリース監査で発見された4件のドキュメント・設定ファイルの不整合を修正する。対象は `.cupola/steering/tech.md` のCLIサブコマンド名、`CHANGELOG.md` の start/stop 機能記載、`README.md` / `README.ja.md` の Unix only 制約明記、`.gitignore` への `.env` 追加である。すべて軽微な変更であり、1 PR でまとめて対応する。

## Requirements

### Requirement 1: steering/tech.md の CLIサブコマンド名更新

**Objective:** 開発者として、steering doc に記載されたCLIサブコマンド名が実装と一致していることを確認したい。そのため、最新の実装 (`start` / `stop` / `init` / `status` / `doctor`) を正確に反映したドキュメントが必要である。

#### Acceptance Criteria

1. The tech.md shall `Subcommands: run / init / status` の記述を `start / stop / init / status / doctor` に更新する。
2. The tech.md shall `cargo run -- run` の使用例を `cargo run -- start` に更新する。
3. When `.cupola/steering/tech.md` を参照したとき、the tech.md shall 実装済みのサブコマンド一覧と一致した内容を提供する。

### Requirement 2: CHANGELOG.md への start/stop 機能記載追加

**Objective:** 開発者・利用者として、CHANGELOG に `cupola start` / `cupola stop` のデーモン機能が記載されていることを確認したい。そのため、リリース前にこれらの機能をリリースノートに反映する必要がある。

#### Acceptance Criteria

1. The docs shall `cupola start --daemon` をv0.1の Added セクションに追記する。
2. The docs shall `cupola stop` をv0.1の Added セクションに追記する。
3. When CHANGELOG.md を参照したとき、the docs shall `start` / `stop` コマンドの説明を含む。

### Requirement 3: README.md / README.ja.md への Unix only 制約明記

**Objective:** 利用者として、cupolaが Unix (macOS / Linux) 専用であることを事前に把握したい。そのため、READMEの要件セクションに対応プラットフォームが明記されている必要がある。

#### Acceptance Criteria

1. The docs shall `README.md` の要件セクションに「Unix (macOS / Linux) only」と明記する。
2. The docs shall `README.ja.md` の要件セクションに「Unix (macOS / Linux) のみ対応」と明記する。
3. If Windows 環境のユーザーが README を参照したとき、the docs shall 動作対象外であることを明示する。
4. The docs shall `cfg(unix)` 依存 (`nix` クレート) に起因する制約であることをコメントまたは記述で示す。

### Requirement 4: .gitignore への .env 追加

**Objective:** 開発者として、GitHub トークン等の機密情報が誤って Git リポジトリにコミットされることを防ぎたい。そのため、`.env` ファイルが `.gitignore` に含まれている必要がある。

#### Acceptance Criteria

1. `.gitignore` shall contain a `.env` ignore entry.
2. When `.env` ファイルが存在するとき、Git shall not track it because it is ignored by `.gitignore`.
3. If `.env` に GitHub トークンが保存されていても、Git shall treat `.env` as an ignored file and not include it in tracked changes.
