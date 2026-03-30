# Research & Design Decisions

## Summary
- **Feature**: `cupola-init-extension`
- **Discovery Scope**: Extension（既存 init コマンドの拡張）
- **Key Findings**:
  - 現在の init は `app.rs` 内のインライン実装で SQLite 初期化のみ実行
  - ファイル生成ロジックは application 層の use case として抽出すべき
  - テンプレートファイルは `.cupola/settings/templates/steering/` に既に存在

## Research Log

### 既存 init 実装の分析
- **Context**: 拡張対象の現在の実装を理解する必要がある
- **Sources Consulted**: `src/bootstrap/app.rs`, `src/adapter/outbound/sqlite_connection.rs`
- **Findings**:
  - `Command::Init` は `app.rs` の match 式で直接処理されている（専用 use case なし）
  - `.cupola/` ディレクトリ作成 → SQLite open → `init_schema()` の3ステップ
  - `init_schema()` は `CREATE TABLE IF NOT EXISTS` を使用し冪等
  - feature_name カラムの ALTER TABLE マイグレーションも含む
- **Implications**: 新機能追加に伴い、init ロジックを use case に抽出して責務を分離すべき

### cupola.toml の構造と設定値
- **Context**: 雛形生成時にどのフィールドを含めるか決定する必要がある
- **Sources Consulted**: `src/bootstrap/config_loader.rs`, `src/domain/config.rs`, `.cupola/cupola.toml`
- **Findings**:
  - 必須フィールド: `owner`, `repo`, `default_branch`
  - オプション: `language`, `polling_interval_secs`, `max_retries`, `stall_timeout_secs`, `max_concurrent_sessions`, `[log]`
  - `CupolaToml` 構造体で serde デシリアライズ
- **Implications**: 雛形は必須フィールドを空欄で、オプションフィールドをコメントアウトで提供する

### steering テンプレートの配置
- **Context**: コピー元テンプレートの存在と内容を確認
- **Sources Consulted**: `.cupola/settings/templates/steering/`
- **Findings**:
  - `product.md`, `structure.md`, `tech.md` の3ファイルが存在
  - これらは cc-sdd のデフォルト steering ドキュメント
  - カスタム steering テンプレートは `steering-custom/` に別管理
- **Implications**: 標準3ファイルのみコピー対象。ディレクトリ不在時は cc-sdd 未インストールとしてスキップ

### .gitignore エントリの設計
- **Context**: cupola が生成する gitignore 対象ファイルの特定
- **Sources Consulted**: プロジェクト構造、cupola の動作仕様
- **Findings**:
  - `.cupola/cupola.db` — SQLite データベース（ローカル状態）
  - `.cupola/logs/` — ログファイル
  - `.cupola/worktrees/` — git worktree 作業ディレクトリ
  - `.cupola/inputs/` — 一時的な入力ファイル
- **Implications**: マーカーコメント `# cupola` で囲んでエントリを管理し、重複検出に利用

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| インライン拡張 | app.rs の既存 match 式に直接追加 | 変更最小、即座に実装可能 | 責務肥大化、テスト困難 | 既存パターンだが拡張には不向き |
| InitUseCase 抽出 | application 層に専用 use case を作成 | テスタブル、責務分離、Clean Architecture 準拠 | ファイル追加が必要 | 推奨：プロジェクトの設計方針に合致 |

## Design Decisions

### Decision: InitUseCase の導入
- **Context**: 現在の init はインライン実装だが、4つの独立した初期化操作を追加する必要がある
- **Alternatives Considered**:
  1. app.rs に直接ロジックを追加 — 変更最小だがテスト困難
  2. InitUseCase を application 層に作成 — Clean Architecture 準拠
- **Selected Approach**: InitUseCase を作成し、各初期化ステップをメソッドとして実装
- **Rationale**: steering の structure.md が示す Clean Architecture パターンに準拠。テスタビリティ向上
- **Trade-offs**: ファイル追加のオーバーヘッドがあるが、保守性が向上
- **Follow-up**: 既存の SQLite 初期化ロジックも use case に移動するか検討

### Decision: ファイル操作のポート定義
- **Context**: ファイルシステム操作をテスタブルにする必要がある
- **Alternatives Considered**:
  1. std::fs を直接使用 — 簡素だがモック不可
  2. FileSystem ポートを定義 — テスタブルだがオーバーヘッド
- **Selected Approach**: std::fs を直接使用（adapter 層内で）
- **Rationale**: init コマンドは単純なファイル操作のみ。過度な抽象化は避ける。統合テストでカバー
- **Trade-offs**: ユニットテストでのモックは困難だが、統合テストで十分カバー可能

### Decision: .gitignore マーカーコメント方式
- **Context**: 重複追記を防ぐ検出メカニズムが必要
- **Alternatives Considered**:
  1. エントリごとに存在チェック — 個別行の解析が必要
  2. マーカーコメントで囲む — ブロック単位で検出可能
- **Selected Approach**: `# cupola` マーカーコメントでブロックを囲み、存在チェックに使用
- **Rationale**: シンプルで確実な重複検出。ユーザーが手動編集した場合も安全
- **Trade-offs**: ユーザーがマーカーを削除すると再追記される可能性あり

## Risks & Mitigations
- テンプレートディレクトリ不在時の適切なスキップ処理 — ログ出力で通知し処理続行
- .gitignore の改行コード差異 — 追記時に既存ファイルの改行コードを尊重
- 並行実行時のファイル競合 — init は通常1回のみ実行のため低リスク。ファイル存在チェックで十分

## References
- Rust std::fs ドキュメント — ファイル操作 API
- toml crate ドキュメント — TOML シリアライズ/デシリアライズ
- プロジェクト steering ドキュメント — Clean Architecture パターン
