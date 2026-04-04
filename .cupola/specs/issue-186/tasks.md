# 実装タスク: doctor の再設計

## Task Format

- [x] 1. DoctorCheckResult にセクションと remediation を追加する
- [x] 1.1 `DoctorSection` enum を追加し、`DoctorCheckResult` に `section` と `remediation` フィールドを追加する
  - `DoctorSection::StartReadiness` と `DoctorSection::OperationalReadiness` の2バリアントを定義する
  - `DoctorCheckResult` に `pub section: DoctorSection` と `pub remediation: Option<String>` を追加する
  - 既存の全チェック関数のシグネチャと戻り値を新構造体に合わせて更新する
  - _Requirements: 1.1, 4.1_

- [x] 2. ConfigLoader ポートを拡張して config validate をサポートする
- [x] 2.1 `ConfigLoadError` に `ValidationFailed` バリアントを追加する
  - `application/port/config_loader.rs` の `ConfigLoadError` に `ValidationFailed { reason: String }` を追加する
  - `MockConfigLoader` のテスト実装に `ValidationFailed` のマッチングを追加する
  - _Requirements: 2.1, 2.2_

- [x] 2.2 `TomlConfigLoader` を `into_config + validate` まで実行するよう更新する
  - `TomlConfigLoader::load()` の実装で `load_toml` → `into_config` → `validate` を順に実行する
  - `ConfigError` と `validate` の `String` エラーをそれぞれ対応する `ConfigLoadError` バリアントにマッピングする
  - _Requirements: 2.1, 2.2_

- [x] 3. Start Readiness チェック関数を実装する
- [x] 3.1 (P) `check_config` を実装する（`check_toml` を置き換える）
  - 不存在・parse 失敗・validate 失敗の各ケースで FAIL を返す
  - `DoctorSection::StartReadiness` を設定する
  - validate 失敗時の remediation を設定する（設定値の修正を案内）
  - _Requirements: 2.1, 2.2, 4.1, 4.3_

- [x] 3.2 (P) `check_git` を Start Readiness・remediation 付きに更新する
  - `DoctorSection::StartReadiness` を設定する
  - FAIL 時の remediation として git インストール手順（`https://git-scm.com/`）を設定する
  - _Requirements: 2.3, 4.3_

- [x] 3.3 (P) `check_github_token` を新規実装する
  - `CommandRunner` 経由で `gh auth token` を実行する
  - 成功時は OK、失敗時は FAIL + remediation (`gh auth login`) を返す
  - `DoctorSection::StartReadiness` を設定する
  - _Requirements: 2.4, 4.3_

- [x] 3.4 (P) `check_claude` を新規実装する
  - `CommandRunner` 経由で `claude --version` を実行する
  - 成功時は OK、失敗時は FAIL + remediation（claude インストール手順）を返す
  - `DoctorSection::StartReadiness` を設定する
  - _Requirements: 2.5, 4.3_

- [x] 3.5 (P) `check_db` を Start Readiness・remediation 付きに更新する
  - `DoctorSection::StartReadiness` を設定する
  - FAIL 時の remediation として `cupola init` を設定する
  - _Requirements: 2.6, 4.2_

- [x] 4. Operational Readiness チェック関数を実装・更新する
- [x] 4.1 (P) `check_assets` を新規実装する
  - `.claude/commands/cupola/` と `.cupola/settings/` の2つのディレクトリ存在を確認する
  - 欠落時は WARN + remediation (`cupola init`) を返す
  - `DoctorSection::OperationalReadiness` を設定する
  - _Requirements: 3.1, 4.2_

- [x] 4.2 (P) `check_steering` を WARN ベースに更新する
  - steering ディレクトリにファイルがない場合、FAIL から WARN に変更する
  - WARN 時の remediation として `cupola init` または `/cupola:steering` の両方を提示する
  - `DoctorSection::OperationalReadiness` を設定する
  - _Requirements: 3.2, 4.4_

- [x] 4.3 (P) `check_gh_label`（agent:ready）を WARN ベースに更新する
  - agent:ready ラベル不在時、FAIL から WARN に変更する
  - WARN 時の remediation として `gh label create agent:ready` を設定する
  - `DoctorSection::OperationalReadiness` を設定する
  - _Requirements: 3.3, 4.3_

- [x] 4.4 (P) `check_weight_labels` を Operational Readiness に更新する
  - `DoctorSection::OperationalReadiness` を設定する
  - WARN 時の remediation として `gh label create weight:light` / `weight:heavy` コマンドを設定する
  - _Requirements: 3.4, 4.3_

- [x] 5. DoctorUseCase::run() を新チェック構成に更新する
- [x] 5.1 `DoctorUseCase::run()` を新チェック関数を呼び出すよう更新する
  - Start Readiness: `check_config`, `check_git`, `check_github_token`, `check_claude`, `check_db`
  - Operational Readiness: `check_assets`, `check_steering`, `check_gh_label`, `check_weight_labels`
  - 既存の `check_gh`（gh CLI 存在確認）を `check_github_token` に統合または削除する
  - _Requirements: 1.1, 1.2, 2.1–2.6, 3.1–3.4_

- [x] 6. CLI 表示ロジックをセクション別・remediation 付きに更新する
- [x] 6.1 `app.rs` の `Command::Doctor` ブランチをセクション別表示に更新する
  - Start Readiness と Operational Readiness のセクションヘッダーを出力する
  - 各チェック結果の後に `remediation` がある場合は `   fix: ` プレフィックスで表示する
  - `has_failure` の判定を Start Readiness セクションの FAIL のみに絞る
  - _Requirements: 1.1–1.4, 4.1–4.5_

- [x] 7. テストを新仕様に合わせて更新・追加する
- [x] 7.1 既存テストを新構造体・severity に合わせて更新する
  - `check_steering` の「ファイルなし」テストを WARN 期待に変更する
  - `check_gh_label` の「ラベルなし」テストを WARN 期待に変更する
  - 全テストで `DoctorCheckResult.section` と `remediation` のアサーションを追加する
  - `doctor_use_case_all_ok_with_mock_loader` の件数を新チェック数に合わせて更新する
  - _Requirements: 5.1_

- [x] 7.2 (P) `check_config` のユニットテストを追加する
  - ValidationFailed を返す `MockConfigLoader` のケースを追加する
  - NotFound / ParseFailed / ValidationFailed の各ケースが FAIL を返すことを検証する
  - OK ケースで section が StartReadiness であることを検証する
  - _Requirements: 2.1, 2.2, 5.2_

- [x] 7.3 (P) `check_github_token` のユニットテストを追加する
  - `MockCommandRunner` で `gh auth token` の成功/失敗ケースを定義してテストする
  - FAIL 時に remediation が `Some` であることを検証する
  - _Requirements: 2.4, 5.2_

- [x] 7.4 (P) `check_claude` のユニットテストを追加する
  - `MockCommandRunner` で `claude --version` の成功/失敗ケースを定義してテストする
  - FAIL 時に section が StartReadiness であることを検証する
  - _Requirements: 2.5, 5.2_

- [x] 7.5 (P) `check_assets` のユニットテストを追加する
  - TempDir を使用して両ディレクトリあり/片方なし/両方なしのケースをテストする
  - 欠落時に WARN + remediation が返ることを検証する
  - _Requirements: 3.1, 5.2_

- [x] 7.6 `DoctorUseCase` 統合テストを更新する
  - `run()` が Start Readiness と Operational Readiness の両セクションを含む結果を返すことを検証する
  - 全チェック OK 時の件数が新チェック追加後の正しい数であることを検証する
  - _Requirements: 5.3, 5.4_
