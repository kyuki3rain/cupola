# 実装計画

- [ ] 1. domain 層に ClaudeCodeEnvConfig value object を実装する
- [ ] 1.1 BASE_ALLOWLIST 定数とパターンマッチ関数を定義する
  - `HOME`、`PATH`、`USER`、`LANG`、`LC_ALL`、`TERM` を `BASE_ALLOWLIST` const として定義する
  - `matches_pattern(key: &str, pattern: &str) -> bool` をサフィックス `*` ワイルドカード対応で実装する
  - `ClaudeCodeEnvConfig` struct（`extra_allow: Vec<String>`）を定義し `Default` を実装する
  - ユニットテストを作成する: 完全一致・サフィックス wildcard マッチ・サフィックス wildcard 非マッチ・中間 wildcard（リテラル扱い）
  - _Requirements: 1.2, 1.3, 3.1, 3.2, 3.3_

- [ ] 1.2 (P) Config struct に claude_code_env フィールドを追加する
  - `src/domain/config.rs` の `Config` struct に `pub claude_code_env: ClaudeCodeEnvConfig` を追加する
  - `Config::default_with_repo()` で `ClaudeCodeEnvConfig::default()` をセットする
  - 既存テストが引き続きパスすることを確認する
  - _Requirements: 2.1_

- [ ] 2. bootstrap 層で [claude_code.env] TOML セクションを解析する
- [ ] 2.1 ClaudeCodeEnvToml と ClaudeCodeToml struct を定義してパースする
  - `[models]`、`[log]` セクションと同様のパターンで `ClaudeCodeEnvToml { extra_allow: Option<Vec<String>> }` と `ClaudeCodeToml { env: Option<ClaudeCodeEnvToml> }` を定義する
  - `CupolaToml` に `claude_code: Option<ClaudeCodeToml>` フィールドを追加する
  - `into_config()` で `ClaudeCodeEnvConfig { extra_allow: ... }` を構築して `Config.claude_code_env` にセットする
  - `[claude_code.env]` セクション未設定時は `extra_allow = []` のデフォルトとなることをテストで確認する
  - _Requirements: 2.1, 2.3, 2.4_

- [ ] 2.2 (P) TOML パースの統合テストを追加する
  - `[claude_code.env]\nextra_allow = ["ANTHROPIC_API_KEY", "CLAUDE_*"]` を含む toml 文字列を `parse_full_toml` テストに追加する
  - セクション未設定時に `extra_allow` が空リストになることを確認するテストを追加する
  - _Requirements: 2.1, 2.3, 2.4_

- [ ] 3. ClaudeCodeProcess に env_clear + allowlist 適用を実装する
- [ ] 3.1 ClaudeCodeProcess コンストラクタと build_command を更新する
  - `ClaudeCodeProcess::new(executable, env_config: ClaudeCodeEnvConfig)` にシグネチャを更新する
  - `build_command()` の冒頭で `cmd.env_clear()` を呼ぶ
  - BASE_ALLOWLIST の各キーについて `std::env::var(key)` が `Ok` の場合のみ `cmd.env(key, val)` する
  - `std::env::vars()` をイテレートし、各キーが `extra_allow` のいずれかのパターンにマッチする場合 `cmd.env(key, val)` する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 3.1, 3.2_

- [ ] 3.2 (P) ClaudeCodeProcess のユニットテスト・統合テストを追加する
  - `build_command_applies_env_clear`: `cmd.get_envs()` が BASE_ALLOWLIST 以外の env var を含まないことを確認する（env 汚染テスト）
  - `build_command_passes_base_allowlist`: BASE_ALLOWLIST の env var が含まれることを確認する
  - `build_command_applies_extra_allow_exact`: exact match パターンが正しく機能することを確認する
  - `build_command_applies_extra_allow_wildcard`: ワイルドカードパターン (`CLAUDE_*`) が正しく機能することを確認する
  - `build_command_excludes_non_matching`: マッチしない env var が含まれないことを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 3.1, 3.2_

- [ ] 3.3 bootstrap の ClaudeCodeProcess 生成箇所を更新する
  - bootstrap（`src/bootstrap/app.rs` 等の DI 箇所）で `ClaudeCodeProcess::new(executable, config.claude_code_env.clone())` を使うよう更新する
  - _Requirements: 1.1, 2.2_

- [ ] 4. (P) cupola init テンプレートに [claude_code.env] セクションを追加する
  - `CUPOLA_TOML_TEMPLATE` 定数の末尾に `[claude_code.env]` セクションをコメントアウト状態で追加する
  - `extra_allow` のコメントアウト候補として `ANTHROPIC_API_KEY`、`CLAUDE_*`、`OPENAI_API_KEY`、`DOCKER_HOST` を含める
  - ワイルドカードサポートの説明コメントを追加する
  - `generate_toml_template()` のロジックは変更しない（定数更新のみ）
  - _Requirements: 4.1, 4.2_

- [ ] 5. doctor コマンドに env allowlist チェックを追加する
- [ ] 5.1 DoctorConfigSummary に claude_code_extra_allow フィールドを追加する
  - `application/port/config_loader.rs` の `DoctorConfigSummary` に `pub claude_code_extra_allow: Vec<String>` を追加する
  - bootstrap の `TomConfigLoader::load()` 実装（または `config_loader.rs` の具体実装）で `extra_allow` を詰めるよう更新する
  - 既存の `MockConfigLoader` テストヘルパーのデフォルト値に `claude_code_extra_allow: vec![]` を追加する
  - _Requirements: 5.1, 5.4_

- [ ] 5.2 check_env_allowlist 関数を DoctorUseCase に追加する
  - `fn check_env_allowlist(summary: &DoctorConfigSummary) -> DoctorCheckResult` を実装する
  - Ok の message に BASE_ALLOWLIST と `extra_allow` パターンの一覧を含める
  - `SENSITIVE_PATTERNS` (GH_TOKEN, GITHUB_TOKEN, AWS_*, AZURE_*, GOOGLE_*, *_API_KEY, *_SECRET, *_TOKEN, *_PASSWORD) と `matches_pattern` で照合し、マッチする extra_allow エントリがあれば CheckStatus::Warn を返す
  - Warn メッセージに対象パターン名と「不要であれば削除を検討してください」というメッセージを含める
  - `DoctorUseCase::run()` の StartReadiness セクション（`check_config()` の結果が Ok の場合のみ）に `check_env_allowlist()` の結果を追加する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 5.3 (P) doctor env allowlist チェックのユニットテストを追加する
  - `check_env_allowlist_no_extra_allow_returns_ok`: `extra_allow` が空のとき Ok を返すことを確認する
  - `check_env_allowlist_with_safe_patterns_returns_ok`: 安全なパターンのみのとき Ok を返すことを確認する
  - `check_env_allowlist_with_gh_token_returns_warn`: `GH_TOKEN` が含まれるとき Warn を返すことを確認する
  - `check_env_allowlist_with_aws_wildcard_returns_warn`: `AWS_*` が含まれるとき Warn を返すことを確認する
  - `doctor_use_case_returns_all_sections` テストを更新して `check_env_allowlist` の結果が含まれることを確認する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_
