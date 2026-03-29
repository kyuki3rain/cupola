# Technology Stack

## Architecture

Clean Architecture（4 レイヤー）。依存方向は内向きのみ。

- **domain**: 純粋ビジネスロジック（State, Event, StateMachine, Issue, Config）。I/O 依存なし
- **application**: ユースケース + ポート定義（trait）。外部依存は trait で抽象化
- **adapter**: 外部接続の実装（GitHub API, SQLite, Claude Code, Git）
- **bootstrap**: DI 配線、設定読み込み、ランタイム起動

## Core Technologies

- **Language**: Rust（Edition 2024）
- **Runtime**: tokio（非同期ランタイム、シグナルハンドリング）
- **Storage**: SQLite（rusqlite、WAL モード）
- **GitHub API**: octocrab（REST）+ reqwest（GraphQL 直接 POST）

## Key Libraries

| 用途 | クレート | パターン |
|------|---------|---------|
| CLI | clap（derive） | サブコマンド: run / init / status |
| GitHub REST | octocrab | personal token 認証 |
| GitHub GraphQL | reqwest + serde_json | 直接 POST、Value パース |
| DB | rusqlite | Arc<Mutex<Connection>>、spawn_blocking |
| ログ | tracing + tracing-appender | 構造化ログ、日付別ファイル出力 |
| エラー | thiserror（domain/app）+ anyhow（adapter/bootstrap） | レイヤーで使い分け |

## Development Standards

### Type Safety
- Rust の型システムによる静的保証
- 全ポートを trait で定義し、実装は adapter に隔離
- State/Event は enum で網羅的パターンマッチ

### Code Quality
- `cargo clippy -- -D warnings`（全警告をエラー扱い）
- `cargo fmt`（rustfmt による統一フォーマット）
- `[lints.clippy] all = "warn"` in Cargo.toml

### Testing
- ユニットテスト: 各モジュール内の `#[cfg(test)]` ブロック
- 統合テスト: `tests/` ディレクトリ、モック adapter 注入
- SQLite テストは in-memory DB を使用

## Development Environment

### Required Tools
- Rust stable（devbox 経由、rustup）
- devbox（Nix ベースの開発環境管理）

### Common Commands
```bash
cargo build          # ビルド
cargo test           # 全テスト実行
cargo clippy         # 静的解析
cargo fmt --check    # フォーマットチェック
cargo run -- run     # polling ループ開始
cargo run -- init    # SQLite スキーマ初期化
cargo run -- status  # Issue 状態一覧
```

## Key Technical Decisions

- **std::process vs tokio::process**: polling ループとの統合が自然な std::process + try_wait() を採用。stdout/stderr は別スレッドで蓄積
- **単一 GitHubClient trait**: REST/GraphQL の区別をアプリケーション層から隠蔽。facade パターンで合成
- **イベントバッチ適用**: polling サイクル内で全イベントを収集し、IssueClosed を最優先で一括適用
