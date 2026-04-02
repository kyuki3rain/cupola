# Implementation Plan

## Task Format Template

- [ ] 1. ログファイル操作のポートトレイトとエラー型を定義する
  - `application/port/` に `LogFilePort` trait を新規定義する（`find_latest_log_file`、`read_tail`、`read_new_content` の3メソッド）
  - ポートが返すエラー型 `LogFileError`（`DirNotFound`、`NoLogFiles`、`Io` バリアント）を `thiserror` で定義する
  - `application/port/mod.rs` にモジュール登録する
  - _Requirements: 1.2, 1.4, 1.5, 2.1, 3.1, 4.3_

- [ ] 2. (P) CLI に `logs` サブコマンドを追加する
  - `Command` enum に `Logs` variant を追加し、`--config`（デフォルト `.cupola/cupola.toml`）・`--follow/-f`・`--lines/-n`（デフォルト20）オプションを定義する
  - clap のパーステスト（デフォルト値、`-f`、`--follow`、`-n 50` の動作）を `#[cfg(test)]` ブロックに追加する
  - _Requirements: 1.6, 2.4, 3.3, 3.4, 4.1_

- [ ] 3. ファイルシステムを使ったログファイル操作を実装する
- [ ] 3.1 最新ログファイルの検索機能を実装する
  - `adapter/outbound/fs_log_file_port.rs` を新規作成し `FsLogFilePort` 構造体を定義する
  - `find_latest_log_file`: `read_dir` でエントリを走査し、ファイル名辞書順最大値を `max_by_key` で取得する（`cupola.YYYY-MM-DD` 形式に対応）
  - ディレクトリ不在 → `LogFileError::DirNotFound`、ファイルなし → `LogFileError::NoLogFiles` を返す
  - `tempfile::TempDir` を使い、複数ファイル存在時に最新が選択されること・ファイルなし時のエラーをユニットテストで確認する
  - _Requirements: 1.4, 1.5, 3.1, 3.2_

- [ ] 3.2 ファイル末尾 N 行読み取り機能を実装する
  - `read_tail`: ファイル末尾からブロック単位で逆走査し、必要最小限の読み取りで末尾 `lines` 行を取得する（大容量ファイルでも全体読み込みを避ける）
  - 実際の行数が `lines` 未満の場合は全行返す（`min(lines, actual)` の動作）
  - `tempfile` を使い、行数丁度・行数未満・行数超過の各ケースをユニットテストで確認する
  - _Requirements: 1.2, 3.3_

- [ ] 3.3 offset ベースの差分読み取り機能を実装する
  - `read_new_content`: `File::open` → `seek(SeekFrom::Start(offset))` → `read_to_end` で offset 以降の内容を取得する
  - 取得した内容を行分割し、新規行リストと新しい offset（= 旧 offset + 読み取りバイト数）を返す
  - `tempfile` を使い、追記前後の差分取得が正しく動作することをユニットテストで確認する
  - `adapter/outbound/mod.rs` に `FsLogFilePort` を登録する
  - _Requirements: 2.1_

- [ ] 4. (P) ログ表示ユースケースを実装する
- [ ] 4.1 通常表示モードを実装する
  - `application/logs_use_case.rs` を新規作成し `LogsUseCase<P: LogFilePort>` を定義する
  - `LogsInput`（`log_dir: Option<PathBuf>`、`lines: usize`、`follow: bool`）と `LogsError`（`NoLogDir`、`File(LogFileError)`）を定義する
  - `execute` の先頭で `log_dir.is_none()` を確認し、未設定の場合は即座に `LogsError::NoLogDir` を返す
  - `find_latest_log_file` と `read_tail` を呼び出し、結果を `output: &mut dyn Write` に書き込む
  - `MockLogFilePort` を使ったユニットテスト（`log_dir=None` のエラー、正常表示、ファイル不在時のエラー伝播）を追加する
  - `application/mod.rs` にモジュール登録する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 3.1, 3.3, 3.4, 4.2_

- [ ] 4.2 フォローモードを実装する
  - `execute` 内で `follow=true` の場合に末尾表示後、100ms 間隔のポーリングループに入る
  - `tokio::select!` で `tokio::time::sleep(Duration::from_millis(100))` と `tokio::signal::ctrl_c()` を競合させ、SIGINT 受信時にループを抜けて正常終了する
  - 各ポーリングで `read_new_content` を呼び出し、新規行を `output` に書き込む
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 5. Bootstrap に `logs` コマンドを配線する
  - `bootstrap/app.rs` の `match cli.command` に `Command::Logs` ブランチを追加する
  - `cupola.toml` を読み込んで `Config.log_dir` を取得し、`LogsInput` に渡す
  - `FsLogFilePort` を構築し `LogsUseCase::new` に注入して `execute` を呼び出す
  - エラー時は `anyhow` で変換してエラーメッセージを stderr に出力し非ゼロ終了する
  - _Requirements: 1.1, 1.3, 4.4_
