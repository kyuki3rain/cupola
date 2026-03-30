# Implementation Plan

## Task Format

- [ ] N. タスク概要
- [ ] N.M サブタスク説明
  - 詳細項目
  - _Requirements: X.X, Y.Y_

---

- [ ] 1. ConfigLoader port の定義と bootstrap 実装

- [ ] 1.1 application 層に ConfigLoader trait と関連型を定義する
  - `DoctorConfigSummary { owner, repo, default_branch }` を application 層に追加する
  - `ConfigLoadError` を thiserror で定義する（NotFound / ParseFailed / MissingField の 3 バリアント）
  - `ConfigLoader: Send + Sync` trait を `application/port/config_loader.rs` に定義し、`fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError>` を宣言する
  - `application/port/mod.rs` に `pub mod config_loader;` を追加して公開する
  - _Requirements: 4.1, 4.2_

- [ ] 1.2 bootstrap 層に TomlConfigLoader を実装する
  - `TomlConfigLoader` 構造体を `bootstrap/config_loader.rs`（または新規モジュール）に定義する
  - `ConfigLoader` trait を `TomlConfigLoader` に実装し、既存の `load_toml()` を内部で呼び出す
  - `CupolaToml` から `DoctorConfigSummary` への変換ロジックを bootstrap 層内で完結させる（owner/repo/default_branch を抽出）
  - `bootstrap/mod.rs` でモジュールを公開する
  - _Requirements: 4.1, 4.2_

---

- [ ] 2. DoctorUseCase の診断ロジックを実装する

- [ ] 2.1 DoctorUseCase の骨格と結果型を定義し、check_toml を実装する
  - `CheckStatus { Ok(String), Fail(String) }` と `DoctorCheckResult { name: String, status: CheckStatus }` を `application/doctor_use_case.rs` に定義する
  - `DoctorUseCase<C: ConfigLoader>` 構造体と `new(config_loader: C) -> Self` を定義する
  - `run(&self, config_path: &Path) -> Vec<DoctorCheckResult>` メソッドを定義し、各チェック関数の呼び出しを組み立てる
  - `check_toml` を実装し、`self.config_loader.load(path)` を呼び出して成功/失敗を `DoctorCheckResult` に変換する（bootstrap への直接依存なし）
  - `application/mod.rs` でモジュールを公開する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 2.2 (P) check_git を実装する（環境依存対応）
  - `git --version` を `std::process::Command` で実行し、終了コードで存在確認する
  - git が存在する場合は `CheckStatus::Ok`、存在しない場合は `CheckStatus::Fail` を返す
  - 失敗メッセージには git のインストール案内を含める
  - 本タスクは 2.1 の DoctorUseCase 骨格に依存するが、2.3-2.6 とは独立して実装できる
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2.3 (P) check_gh を実装する（未インストール vs 認証失敗の区別）
  - `gh --version` を実行し、`io::Error::kind() == ErrorKind::NotFound` で未インストールを検出する
  - 未インストールの場合は `CheckStatus::Fail`（インストール手順案内）を返す
  - インストール済みの場合は `gh auth status` を実行し、終了コードが非 0 なら `CheckStatus::Fail`（`gh auth login` 案内）を返す
  - `GhPresence { NotInstalled, InstalledButUnauthorized, Ready }` 内部 enum で分岐を整理する
  - 本タスクは 2.1 の DoctorUseCase 骨格に依存するが、2.2/2.4-2.6 とは独立して実装できる
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 2.4 (P) check_gh_label を実装する（agent:ready の JSON パース）
  - `gh label list --json name` を実行して stdout を取得する
  - `#[derive(serde::Deserialize)] struct LabelItem { name: String }` を定義し、`serde_json::from_str::<Vec<LabelItem>>(&stdout)` でパースする
  - `item.name == "agent:ready"` が存在する場合のみ `CheckStatus::Ok` を返す
  - JSON パース失敗時は `CheckStatus::Fail`（エラーメッセージ付き）を返す
  - `stdout.contains("agent:ready")` による文字列マッチは使用しない
  - 本タスクは 2.1 に依存するが 2.2/2.3/2.5/2.6 とは独立して実装できる
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 2.5 (P) check_steering を実装する（ファイル限定カウント）
  - steering ディレクトリ（`.cupola/steering/`）を `std::fs::read_dir` で走査する
  - `entry.file_type()?.is_file()` でフィルタし、通常ファイルのみをカウントする
  - カウントが 0 の場合（空ディレクトリ、サブディレクトリのみ、`.DS_Store` のみ等）は `CheckStatus::Fail` を返す
  - `read_dir().next().is_some()` による不正確な確認は使用しない
  - 本タスクは 2.1 に依存するが 2.2-2.4/2.6 とは独立して実装できる
  - _Requirements: 6.1, 6.2, 6.3_

- [ ] 2.6 (P) check_db を実装する
  - `.cupola/cupola.db` ファイルの存在を `Path::exists()` で確認する
  - 存在しない場合は `CheckStatus::Fail`（`cupola init` を実行するよう案内）を返す
  - 本タスクは 2.1 に依存するが 2.2-2.5 とは独立して実装できる
  - _Requirements: 2.4_

---

- [ ] 3. CLI サブコマンド統合と表示ロジックの実装

- [ ] 3.1 cli.rs に Doctor サブコマンドを追加する
  - `Command::Doctor { config: PathBuf }` を `clap` の `Subcommand` enum に追加する
  - `--config` オプションのデフォルト値を `.cupola/cupola.toml` とする
  - `parse_doctor_command` の clap テストケースを追加する
  - _Requirements: 7.1_

- [ ] 3.2 bootstrap/app.rs に Doctor ハンドラを統合する（Err 返却・表示ロジック）
  - `Command::Doctor { config }` の match arm を `bootstrap/app.rs` に追加する
  - `TomlConfigLoader` をコンストラクタ注入して `DoctorUseCase::new(loader)` を構築する
  - `use_case.run(&config)` の結果を走査し、`CheckStatus::Ok` → `✅ {msg}`、`CheckStatus::Fail` → `❌ {msg}` で表示する（表示ロジックはこのハンドラが担う）
  - いずれかの結果が `Fail` の場合は `Err(anyhow::anyhow!("doctor checks failed"))` を返す
  - `std::process::exit(1)` は使用しない（tracing guard の drop を保証するため）
  - _Requirements: 4.3, 7.1, 7.2, 7.3_

---

- [ ] 4. ユニットテストの追加

- [ ] 4.1 (P) toml チェックのユニットテストを追加する（tempdir ベース）
  - `tempfile::TempDir` を使って有効な `cupola.toml` を作成し、`check_toml` が `Ok` を返すことを検証する
  - toml ファイルが存在しない場合に `Fail` を返すことを検証する
  - 必須フィールド（`default_branch`）が欠けた toml で `Fail` を返すことを検証する
  - 本タスクはタスク 3 と同時並行で実装できる
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 4.2 (P) steering チェックのユニットテストを追加する（tempdir ベース）
  - `TempDir` に `.md` ファイルを 1 件作成し、`check_steering` が `Ok` を返すことを検証する
  - `TempDir` にサブディレクトリのみ作成し、`Fail` を返すことを検証する
  - 空の `TempDir` で `Fail` を返すことを検証する
  - 本タスクはタスク 3 と同時並行で実装できる
  - _Requirements: 2.4, 6.1, 6.2, 6.3_

- [ ] 4.3 (P) db チェックのユニットテストを追加する（tempdir ベース）
  - `TempDir` にダミーの db ファイルを作成し、`check_db` が `Ok` を返すことを検証する
  - ファイルなしの `TempDir` で `check_db` が `Fail` を返すことを検証する
  - 本タスクはタスク 3 と同時並行で実装できる
  - _Requirements: 2.4_

- [ ] 4.4 (P) git テストの環境依存を解消する
  - `git --version` を実行し、コマンドが存在しない場合はテスト内でランタイムスキップ（早期 `return`）する
  - git が存在する環境では `check_git` が `Ok` を返すことをアサートする
  - `#[ignore]` ではなくランタイムスキップを使用して通常の `cargo test` でも問題なく実行できるようにする
  - 本タスクはタスク 3 と同時並行で実装できる
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 4.5 (P) agent:ready ラベルの JSON パーステストを追加する
  - `[{"name":"agent:ready"}]` を入力として `check_gh_label` 相当のロジックが `Ok` を返すことを検証する
  - `[{"name":"not-agent:ready"}]` で誤検知なしに `Fail` を返すことを検証する
  - 不正 JSON（空文字列等）で `Fail` を返すことを検証する
  - 本タスクはタスク 3 と同時並行で実装できる
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 4.6 (P) DoctorUseCase の統合テストをモック ConfigLoader で追加する
  - `ConfigLoader` trait のモック実装（`MockConfigLoader`）を test モジュール内に定義する
  - `DoctorUseCase::new(MockConfigLoader)` を使って bootstrap 層に依存せずにテストできることを確認する
  - 全チェックが `Ok` のシナリオと、toml/steering のいずれかが `Fail` のシナリオを検証する
  - _Requirements: 4.4_
