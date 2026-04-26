# doctor-quality-improvement サマリー

## Feature
`cupola doctor` コマンド (PR #46) の Copilot レビュー指摘 7 件を一括修正。Clean Architecture 遵守・テスト強化・エラーハンドリング改善。

## 要件サマリ
1. git 未インストール環境でもテストが落ちないようランタイムスキップを導入。
2. toml / steering / db チェックに tempdir ベースのユニットテストを追加。
3. `gh` 未インストールと認証失敗を区別して独立したガイダンスを表示。
4. `DoctorUseCase` の `bootstrap` 層直接依存を port trait 経由に是正。
5. `agent:ready` ラベル検出を `serde_json` による厳密パースに変更し誤検知を排除。
6. steering チェックを `is_file()` + `.` 先頭除外でファイルのみカウント。
7. `std::process::exit(1)` を `Err(anyhow!...)` 返却に変更し tracing guard の drop を保証。

## アーキテクチャ決定
- `application/port/config_loader.rs` に `ConfigLoader` trait を新設し `bootstrap` に `TomlConfigLoader` を実装、依存逆転を実現。`CupolaToml` は bootstrap 層内に閉じ、最小情報 DTO `DoctorConfigSummary` のみを application 層に提供。
- 表示ロジック (✅/❌) は adapter/inbound ハンドラへ移動し `DoctorUseCase` は `Vec<DoctorCheckResult>` を返す純粋オーケストレータに限定。
- git テストは `#[ignore]` ではなくランタイムスキップ（早期 return）を採用し通常の `cargo test` で検証可能に。
- `exit(1)` 廃止は tracing-appender の `WorkerGuard` の drop 保証（ログフラッシュ漏れ防止）が動機。

## コンポーネント
- `application/port/config_loader.rs`: `ConfigLoader`, `DoctorConfigSummary`, `ConfigLoadError`
- `application/doctor_use_case.rs`: `DoctorUseCase<C>`, `CheckStatus`, `DoctorCheckResult`, `GhPresence`, `LabelItem`
- `bootstrap/toml_config_loader.rs`: `TomlConfigLoader`
- `adapter/inbound/cli.rs`: `Command::Doctor { config }`
- `bootstrap/app.rs`: Doctor arm の DI + 表示 + Err 返却

## 主要インターフェース
- `trait ConfigLoader { fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError>; }`
- `DoctorUseCase::run(&self, config_path: &Path) -> Vec<DoctorCheckResult>`
- `enum CheckStatus { Ok(String), Warn(String), Fail(String) }`

## 学び/トレードオフ
- 中間 DTO 導入で変換コードは増えるが bootstrap 型の漏出を防げる。
- `gh` チェックは 2 段階（`gh --version` → `gh auth status`）で実装し exit code を優先。
- `.DS_Store` 誤検知回避のため `.` 先頭ファイル名も除外。
