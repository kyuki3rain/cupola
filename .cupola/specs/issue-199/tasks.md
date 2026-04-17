# 実装計画

- [x] 1. CLIフラグの追加
- [x] 1.1 `Command::Init` に `--upgrade` フラグを追加する
  - `src/adapter/inbound/cli.rs` の `Init` variant に `upgrade: bool` フィールドを追加する
  - `#[arg(long, default_value_t = false)]` アトリビュートを付与してデフォルトを `false` にする
  - 既存の `parse_init_command` テストを更新し、`upgrade=false` のデフォルト値を確認するテストを追加する
  - `cupola init --upgrade` のパーステスト、`--upgrade --agent claude-code` の複合フラグテストを追加する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. `FileGenerator` ポートのシグネチャ変更
- [x] 2.1 `FileGenerator` トレイトのメソッドシグネチャに `upgrade: bool` を追加する
  - `src/application/port/file_generator.rs` の `install_claude_code_assets` と `append_gitignore_entries` に `upgrade: bool` パラメータを追加する
  - `generate_toml_template`・`generate_spec_directory`・`generate_spec_directory_at` は変更しない（ユーザー所有ファイルのため）
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3_

- [x] 3. `InitUseCase` へのフラグスレッド
- [x] 3.1 `InitUseCase` に `upgrade: bool` フィールドを追加し、`FileGenerator` メソッドに渡す
  - `src/application/init_use_case.rs` の構造体に `upgrade: bool` フィールドを追加する
  - `new()` コンストラクタに `upgrade: bool` パラメータを追加する
  - `run()` 内の `file_gen.install_claude_code_assets()` を `file_gen.install_claude_code_assets(self.upgrade)` に変更する
  - `run()` 内の `file_gen.append_gitignore_entries()` を `file_gen.append_gitignore_entries(self.upgrade)` に変更する
  - `generate_toml_template()` の呼び出しは変更しない
  - `upgrade=true`・`upgrade=false` それぞれで `FileGenerator` が適切に呼ばれることを確認するテストを追加する
  - 既存テスト内の `InitUseCase::new()` 呼び出し箇所に `upgrade: false` を追加して更新する
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3_

- [x] 4. `InitFileGenerator` のアップグレードロジック実装
- [x] 4.1 (P) `install_claude_code_assets` にアップグレード時の差分比較ロジックを実装する
  - `src/adapter/outbound/init_file_generator.rs` の `install_claude_code_assets` シグネチャを `(upgrade: bool)` に変更する
  - `upgrade=true` の場合も既存ファイルの内容と埋め込み `CLAUDE_CODE_ASSETS` の内容を比較し、差分がある場合のみ上書きする
  - `upgrade=true` かつ差分がない場合は書き込みを行わずスキップし、`"already up to date"` を判定できる状態にする
  - `upgrade=false` の場合は既存の `exists()` チェックロジックを維持する
  - `FileGenerator` impl の委譲メソッドシグネチャも更新する
  - テスト: `upgrade=true` で差分ありなら既存ファイルの内容が最新版に変わること、差分なしならスキップされること、`upgrade=false` で既存ファイルがスキップされること
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x] 4.2 (P) `append_gitignore_entries` にアップグレード時のセクション置換ロジックを実装する
  - `src/adapter/outbound/init_file_generator.rs` の `append_gitignore_entries` シグネチャを `(upgrade: bool)` に変更する
  - `upgrade=true` かつ `GITIGNORE_MARKER` が存在する場合: 行単位で分割しCupolaブロック（マーカー行から次の空行まで）を `GITIGNORE_ENTRIES` で置換する
  - `upgrade=true` かつマーカーが存在しない場合: 末尾追記する（`upgrade=false` と同じ）
  - `upgrade=false` の場合は既存の動作を維持する
  - CRLF/LF 改行コードの保持ロジックをアップグレードパスにも適用する
  - `FileGenerator` impl の委譲メソッドシグネチャも更新する
  - テスト: Cupolaセクション置換・ユーザーエントリ保護・マーカーなし時の追記・CRLF保持・エッジケース（末尾改行なし、Cupolaブロックが最終行）
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 5. Bootstrap の接続と結果表示の更新
- [x] 5.1 `app.rs` の Init ハンドラを `upgrade` フラグに対応させる
  - `src/bootstrap/app.rs` の `Command::Init { agent }` パターンマッチを `Command::Init { agent, upgrade }` に変更する
  - `InitUseCase::new(...)` の呼び出しに `upgrade` を渡す
  - `agent assets:` の出力メッセージを `upgrade` フラグに応じて切り替える（`upgrade=true` では `"upgraded"` / `"already up to date"`、`upgrade=false` では従来の `"installed"` / `"skipped"`）
  - `.gitignore` の出力メッセージも同様に切り替える
  - _Requirements: 5.1, 5.2, 5.3_
