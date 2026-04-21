# リサーチ・設計判断ログ

---
**目的**: 技術設計を支える調査結果・アーキテクチャ検討・判断根拠を記録する。

---

## サマリー

- **機能**: issue-321 — Claude Code 子プロセスの env whitelist 最小化
- **ディスカバリースコープ**: Extension（既存システムへの拡張）
- **主要な発見事項**:
  - `ClaudeCodeProcess::build_command()` は `Command::env_clear()` を一切呼んでおらず、全 env が Claude Code に渡る状態
  - `Config` 構造体（domain 層）は `claude_code` 関連フィールドを持たず、今回新規に追加が必要
  - `DoctorUseCase` は `ConfigLoader` port 経由で `DoctorConfigSummary` を取得しており、env 情報を追加するには port 側の拡張が必要
  - `CUPOLA_TOML_TEMPLATE` は `init_file_generator.rs` 内に `const` として定義されており、セクション追加で対応可能

## リサーチログ

### `ClaudeCodeProcess` の現状分析

- **コンテキスト**: env 継承の問題箇所を特定するため
- **調査したファイル**: `src/adapter/outbound/claude_code_process.rs:174-189`
- **発見事項**:
  - `build_command()` は `Command::new()` 後に `env_clear()` を呼ばない
  - `cmd.env(key, val)` も一切呼ばれていない
  - `Stdio::piped()` で stdout/stderr は取得しているが、env はデフォルト継承のまま
- **影響**: 実装では `build_command()` を更新して `env_clear()` + allowlist 適用を追加する

### `Config` / `CupolaToml` の構造

- **コンテキスト**: config 追加の統合箇所を特定するため
- **調査したファイル**: `src/domain/config.rs`, `src/bootstrap/config_loader.rs`
- **発見事項**:
  - `Config` は domain value object として clean architecture に準拠
  - `CupolaToml` → `Config` の変換は `into_config()` で行われる
  - `[models]`、`[log]` と同じパターンで `[claude_code.env]` セクションを追加できる
  - `Config` への新フィールド追加は `default_with_repo()` も更新が必要
- **影響**: `ClaudeCodeEnvConfig` を domain 層の value object として定義し、`Config` に追加

### `DoctorUseCase` の拡張方針

- **コンテキスト**: doctor チェックへの統合方式を決定するため
- **調査したファイル**: `src/application/doctor_use_case.rs`, `src/application/port/config_loader.rs`（推定）
- **発見事項**:
  - `DoctorConfigSummary` に `owner`, `repo`, `default_branch` のみが含まれる
  - `check_config()` で config ロードの成否のみを確認しており、config 内容は表示しない
  - 新しい `check_env_allowlist()` 関数を追加し、`DoctorConfigSummary` 経由で env 情報を取得する方針が最小変更
- **影響**: `DoctorConfigSummary` に `claude_code_extra_allow: Vec<String>` を追加する

### 既知の危険な env パターン定義

- **コンテキスト**: doctor warn の対象を決定するため
- **発見事項**:
  - Issue では `GH_TOKEN`、`AWS_SECRET_ACCESS_KEY` を例示
  - 一般的な secrets: `*_TOKEN`、`*_SECRET`、`*_API_KEY`、`*_PASSWORD` の suffix パターン
  - `AWS_*` は大量の secrets を含む可能性
  - 過剰な警告は UX を損ねるため、既知の高リスクパターンに絞る
- **影響**: `SENSITIVE_PATTERNS` として hardcode するが、実装はシンプルに保つ

## アーキテクチャパターン評価

| オプション | 説明 | 利点 | リスク・制限 | 採用 |
|-----------|------|------|------------|------|
| `ClaudeCodeProcess` に env config を DI | `new(executable, env_config)` で `ClaudeCodeEnvConfig` を受け取る | `ClaudeCodeRunner` trait を変更不要、bootstrap での注入が容易 | — | ✅ |
| `ClaudeCodeRunner` trait に env config を渡す | `spawn(prompt, working_dir, schema, model, env_config)` | trait で統一 | trait 変更で全実装・テストに影響 | ❌ |
| `DoctorConfigSummary` 拡張 | `claude_code_extra_allow` フィールドを追加 | 既存 port パターンと整合 | — | ✅ |
| `DoctorUseCase` に `Config` を直接渡す | config_loader port をバイパス | シンプル | レイヤー違反（application が bootstrap 依存） | ❌ |

## 設計判断

### 判断: `ClaudeCodeEnvConfig` を domain 層 value object として定義

- **コンテキスト**: env config をどのレイヤーで管理するか
- **検討した選択肢**:
  1. domain 層の value object (`src/domain/claude_code_env.rs`)
  2. adapter 層のローカル struct (`ClaudeCodeProcess` 内部定義)
- **採択アプローチ**: domain 層 value object
- **根拠**: `Config` が domain に属するため一貫性がある。`BASE_ALLOWLIST` は機密性の低い要件であり domain に置く意味がある。adapter 層のみで持つと DoctorUseCase（application 層）からアクセスできない。
- **トレードオフ**: domain に `Vec<String>` が増えるが、これは設定値であり純粋なビジネスロジックと矛盾しない

### 判断: `matches_pattern` を純粋関数（フリー関数）として実装

- **コンテキスト**: ワイルドカードマッチのロジックをどこに置くか
- **採択アプローチ**: `ClaudeCodeEnvConfig` に関連付けたが、実装はシンプルな static/associated function
- **根拠**: glob crate は不要。サフィックス `*` のみサポートで十分（Issue 指定）。単純な `strip_suffix + starts_with` で実装できる。
- **トレードオフ**: 柔軟性を犠牲にしてシンプルさを優先

### 判断: 危険パターンの定義方法

- **コンテキスト**: doctor warn で何を「危険」とみなすか
- **採択アプローチ**: 既知の高リスクパターンを `SENSITIVE_PATTERNS` として `doctor_use_case.rs` に hardcode
- **根拠**: ユーザー設定可能にすると設定が複雑化する。既知の secrets（`GH_TOKEN`、`GITHUB_TOKEN`、`AWS_*`、`*_API_KEY`、`*_SECRET`、`*_TOKEN`、`*_PASSWORD`）のサフィックスパターンで十分な警戒効果がある
- **トレードオフ**: false positive あり得るが、warn は強制ではなくアドバイスのみ

## リスクと緩和策

- **BASE_ALLOWLIST に必要な var が不足する可能性** — 実際の Claude Code 動作確認が必要。`TMPDIR` 等が不足すると一部の MCP ツールが失敗する可能性がある。統合テストで早期検出する。
- **env_clear が devbox 環境での PATH を壊す可能性** — `PATH` は BASE_ALLOWLIST に含まれるため問題なし。ただし devbox 管理の特殊なパスが `PATH` に入っていれば継承される。
- **doctor の env allowlist チェックが config ロード失敗時にパニックする可能性** — `config_loader.load()` 失敗時は env チェックをスキップする設計で回避する（Requirement 5.4）。

## 参考文献

- `std::process::Command::env_clear()` — Rust std 標準 API、外部依存なし
- Issue #321 本文 — 推奨実装の詳細仕様
- Issue #319 (token シークレット型化)、Issue #320 (hash 改変検知) — 多層防御の他の層
