# gh-token-integration サマリー

## Feature
自前実装の `resolve_github_token()` を dtolnay の `gh-token` クレート (v0.1) に置き換え、トークン取得ロジックを簡素化。

## 要件サマリ
1. `Cargo.toml` に `gh-token = "0.1"` を追加、`serde_yaml` 0.9 との競合なくビルドできる。
2. `github_rest_client.rs` から `resolve_github_token()` とそのテスト (`resolve_token_from_env`) を削除。
3. `bootstrap/app.rs` の呼び出しを `gh_token::get()?` に置き換え。
4. トークン取得の 4 段階フォールバック（`GH_TOKEN` → `GITHUB_TOKEN` → `~/.config/gh/hosts.yml` → `gh auth token`）を実現。
5. 既存の `anyhow` ベースエラーハンドリングを維持し、`cargo test` / `cargo clippy -D warnings` を通す。

## アーキテクチャ決定
- ラッパー関数を維持する案は「保守コスト削減」という主目的に反するため不採用。`gh_token::get()` が既存と同じ `anyhow::Result<String>` を返すため、`resolve_github_token()` を完全削除して直接呼び出しに変更。
- `gh-token` はインフラユーティリティとして bootstrap 層からのみ依存させ、adapter 層への依存は追加しない（Clean Architecture の依存方向維持）。
- エラーメッセージは `gh-token` 提供のものをそのまま `?` で伝搬。

## コンポーネント
- `Cargo.toml`: `gh-token` 依存追加
- `src/bootstrap/app.rs`: `resolve_github_token` の use 削除、`gh_token::get()?` に置換
- `src/adapter/outbound/github_rest_client.rs`: `resolve_github_token` 関数と関連テスト削除

## 主要インターフェース
- `gh_token::get() -> anyhow::Result<String>`（外部クレート提供）

## 学び/トレードオフ
- `serde_yaml` 0.9 (archived) への間接依存が発生するが、dtolnay が使用継続する 141 行の小クレートであり実用リスクは低い。
- `resolve_token_from_env` テストは削除し、`gh_token` 内部テストに委ねる形になる。
- ccforge での実績あり。
