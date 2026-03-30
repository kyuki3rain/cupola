# Research & Design Decisions

---
**Purpose**: 設計調査の結果・アーキテクチャ評価・決定理由を記録する。

---

## Summary

- **Feature**: `doctor-quality-improvement`
- **Discovery Scope**: Extension（既存 cupola doctor コマンドへの改善）
- **Key Findings**:
  - PR #46 で実装された doctor コマンドは、この worktree ブランチには含まれていない。設計は Issue #48 の 7 件の改善要件を基にゼロから整理する。
  - 現在の codebase には `DoctorUseCase` が存在しないため、application 層に新規追加する。`bootstrap/config_loader.rs` の `load_toml` を port trait 経由で抽象化することが依存逆転の核心。
  - `std::process::exit(1)` 問題は `tracing-appender` の非同期 guard の `drop` を飛ばすことで、ログバッファが未フラッシュになるリスクがある。

## Research Log

### ConfigLoader の設計（依存逆転）

- **Context**: `DoctorUseCase` が `bootstrap::config_loader::load_toml` を直接呼び出すと、application → bootstrap の禁止依存方向が発生する。
- **Sources Consulted**: 既存コード `src/bootstrap/config_loader.rs`、`src/application/port/` 配下の既存 trait 定義群
- **Findings**:
  - 既存 port 群（`GitHubClient`, `IssueRepository` 等）はすべて application 層に trait を定義し、adapter 層で実装する構成。
  - `CupolaToml` 型は bootstrap 層にあるが、doctor チェックで必要な情報（owner, repo, default_branch の存在確認）は boolean レベルの結果として返すことも可能。
  - 最もシンプルな port は `fn load(&self, path: &Path) -> Result<(), ConfigCheckError>` のような検証結果のみを返すもの。ただし将来の拡張性を考えると設定値を返す方が望ましい。
- **Implications**: `application/port/config_loader.rs` に `ConfigLoader` trait を新規定義し、`bootstrap` の `TomlConfigLoader` が実装する形にする。

### gh コマンドのエラー区別

- **Context**: `gh` の `CommandNotFound` vs 認証エラーを区別する手法の調査。
- **Findings**:
  - `which::which("gh")` または `std::process::Command::new("gh").arg("--version")` の `io::Error::kind() == ErrorKind::NotFound` でコマンド存在確認。
  - `gh auth status` の終了コードが非 0 かつ stderr に "not logged into" などが含まれる場合は認証失敗。
  - Rust では `std::process::Command` の出力から exit status を取得できる。
- **Implications**: doctor チェック内で 2 段階確認を実装する: ①コマンド存在確認 → ②認証状態確認。

### agent:ready ラベルの JSON パース

- **Context**: `stdout.contains("agent:ready")` は "not-agent:ready" 等にも一致する誤検知リスクがある。
- **Findings**:
  - `gh label list --json name` は `[{"name":"agent:ready"},...]` 形式の JSON を返す。
  - `serde_json::from_str::<Vec<LabelItem>>(&stdout)` でパースし、`item.name == "agent:ready"` を確認するのが正確。
- **Implications**: `LabelItem { name: String }` 型を application 層内に定義してパース。

### steering ディレクトリのファイルカウント

- **Context**: `read_dir().next().is_some()` はディレクトリエントリ（サブディレクトリ・`.DS_Store` 等）もカウントしてしまう。
- **Findings**:
  - `entry.file_type()?.is_file()` でフィルタすることでファイルのみ確認可能。
  - `.DS_Store` はファイルだが、`entry.file_name().to_string_lossy().starts_with('.')` で除外する方針も検討したが、隠しファイル除外は過剰設計と判断。steering ファイルは `.md` 等であり、`.DS_Store` のみの場合は実質空とみなすべきとする Issue の意図を採用。
- **Implications**: `is_file()` フィルタのみで要件を満たす。

### process::exit(1) の廃止

- **Context**: `tracing-appender` は非同期ライタを使用しており、`WorkerGuard` の `drop` が呼ばれないとバッファがフラッシュされない。
- **Findings**:
  - `std::process::exit(1)` は `Drop` トレイトを実行せずにプロセスを終了させる。
  - `Err(anyhow!(...))` を返して `main` の `?` 演算子でエラーを伝播させると、スタックアンワインドが行われ `_guard` が適切に `drop` される。
- **Implications**: doctor ハンドラは `Err(anyhow!(...))` を返す。main 側のエラーハンドリングでメッセージ出力と終了コードを制御。

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|----------|------|------|-----------|------|
| A: application 層に設定ロジックを移動 | `CupolaToml` と `load_toml` を application 層に移動 | シンプル | bootstrap の他のユーザーが影響を受ける可能性 | 大規模な移動が必要 |
| B: port trait 経由（採用） | `ConfigLoader` trait を application に定義、bootstrap で実装 | Clean Architecture に準拠、テスト容易 | trait 定義のボイラープレート | 既存 port 群と同パターン |
| C: doctor 専用の設定検証 trait | `DoctorConfigChecker` として OK/NG のみ返す | 最小 | 汎用性ゼロ | 過剰なドメイン分離 |

**選択**: Option B（port trait 経由）— 既存の Clean Architecture パターンと一致し、モックテストが容易。

## Design Decisions

### Decision: `ConfigLoader` port の返却型

- **Context**: `DoctorUseCase` が設定を読み込む際に何を返すべきか
- **Alternatives Considered**:
  1. `Result<(), ConfigCheckError>` — 検証結果のみ
  2. `Result<CupolaToml, ConfigError>` — フル設定値を返す（bootstrap 型を application 層に移動）
  3. `Result<DoctorConfigSummary, ConfigError>` — 必要なフィールドのみの中間 DTO
- **Selected Approach**: Option 3。`DoctorConfigSummary { owner: String, repo: String, default_branch: String }` を application 層に定義し、port がこれを返す。
- **Rationale**: bootstrap 型（`CupolaToml`）を application 層に持ち込まずに済み、doctor が必要な最小情報のみを取得できる。
- **Trade-offs**: 追加 DTO が必要だが、依存方向の清潔さと引き換えに許容範囲。
- **Follow-up**: 実装時に `CupolaToml` のフィールドと `DoctorConfigSummary` の整合性を確認。

### Decision: `DoctorCheckResult` の設計

- **Context**: 複数チェック（toml/git/gh/steering/db/label）の結果をどう表現するか
- **Selected Approach**: `enum CheckStatus { Ok(String), Fail(String) }` と `struct DoctorCheckResult { name: String, status: CheckStatus }` のベクタとして `DoctorUseCase` が返す。
- **Rationale**: 各チェックの名前・結果・メッセージを統一的に扱える。表示フォーマット（✅/❌）はハンドラ側で決定。

### Decision: git テストの環境依存解消方法

- **Selected Approach**: テスト内で `which::which("git").is_ok()` または `std::process::Command::new("git").arg("--version").output()` を確認し、git が存在しない場合は `return` でスキップ。`#[ignore]` ではなくランタイムスキップとする。
- **Rationale**: `#[ignore]` だと CI で明示的に `-- --ignored` を渡さないと実行されず、通常の `cargo test` で検証できない。

## Risks & Mitigations

- `gh label list --json name` の JSON スキーマが将来変更される — `serde_json` のデシリアライズを lenient（`#[serde(rename_all)]` なし、追加フィールド無視）にしておくことで対応
- `ConfigLoader` port の追加により `bootstrap/app.rs` の `Doctor` ハンドラへの DI が必要 — コンストラクタ注入でシンプルに解決
- tempdir テストで OS によるパスの違いが出る可能性 — `tempfile::TempDir` を使えば自動クリーンアップ、クロスプラットフォーム対応も問題なし

## References

- [Clean Architecture - dependency rule](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [Rust tempfile crate](https://docs.rs/tempfile/latest/tempfile/) — tempdir を使ったテスト
- [tracing-appender WorkerGuard](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.WorkerGuard.html) — guard の drop 必須
