# Implementation Plan

- [ ] 1. CLI にバージョンフラグを追加する
- [ ] 1.1 `Cli` 構造体の `#[command(...)]` アトリビュートに `version` を追加する
  - `src/adapter/inbound/cli.rs` の `Cli` 構造体に `#[command(version)]` キーワードを付与し、`--version` と `-V` フラグを自動登録する
  - clap が `env!("CARGO_PKG_VERSION")` を自動適用し、`cupola 0.1.0` 形式でバージョンを表示できることを手動実行で確認する
  - 既存の `start` / `stop` / `init` / `status` / `doctor` サブコマンドの動作に影響がないことを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.1, 3.2_

- [ ] 2. バージョンフラグの動作をテストで検証する
- [ ] 2.1 `--version` フラグのユニットテストを追加する
  - `Cli::try_parse_from(["cupola", "--version"])` が `ErrorKind::DisplayVersion` を返すことを検証する
  - エラーメッセージに `"cupola"` が含まれることを確認する
  - エラーメッセージに現在のバージョン番号（`env!("CARGO_PKG_VERSION")` の値）が含まれることを確認する
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2.2 `-V` ショートフラグのユニットテストを追加する
  - `Cli::try_parse_from(["cupola", "-V"])` が `ErrorKind::DisplayVersion` を返すことを検証する
  - `--version` と同一のエラーメッセージが得られることを確認する
  - _Requirements: 2.1, 2.2_
