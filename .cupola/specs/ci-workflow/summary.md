# ci-workflow

## Feature
GitHub Actions による基本 CI ワークフロー `.github/workflows/ci.yml` を新規作成し、main ブランチ向け PR および main への push 時に fmt / clippy / unit test を自動実行してコード品質を担保する。

## 要件サマリ
- トリガー: `pull_request`（main 向け）と `push`（main）。
- 単一ジョブ `check` を `ubuntu-latest` で逐次実行。
- ステップ構成: checkout → `dtolnay/rust-toolchain@stable`（rustfmt, clippy 付）→ `Swatinem/rust-cache` → `cargo fmt -- --check` → `cargo clippy --all-targets` → `cargo test --lib -- --test-threads=1`。
- ワークフローレベル env: `CARGO_TERM_COLOR=always` / `RUSTFLAGS="-D warnings"`（clippy と build の警告をエラー化）。
- `--test-threads=1` で SQLite 等の共有リソース競合を回避。
- 各ステップの非ゼロ終了でジョブ全体を失敗扱い（GitHub Actions デフォルト動作）。

## アーキテクチャ決定
- **ジョブ構成**: 単一ジョブ逐次 vs. マルチジョブ並列で検討。単一ジョブを採用。理由: プロジェクト規模が小さく並列化のメリットが薄い、rust-cache の共有が容易、シンプル。ビルド時間が長くなったら将来マルチジョブ化を検討。
- **アクションバージョン固定**: サプライチェーン対策として `actions/checkout@v4` と `Swatinem/rust-cache@v2` はコミット SHA 固定。`dtolnay/rust-toolchain@stable` はリポジトリオーナーが信頼できるためタグ指定を許容（例外）。
- **`RUSTFLAGS="-D warnings"` の設定位置**: ステップ単位ではなくワークフローレベル env に設定し、clippy と build の双方を一貫してエラー化。
- **テスト範囲**: 本フェーズでは `--lib` のみ。統合テスト・E2E・マルチジョブ並列化・CD パイプライン・Dependabot は Non-Goals として別スコープへ。
- **既存参考定義の活用**: Issue 本文の参考定義が Rust CI ベストプラクティスに沿っていたため、大きな修正なく採用。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `.github/workflows/ci.yml` | Infrastructure / CI | PR・push 時の fmt/clippy/test 自動実行定義 |

## 主要インターフェース
- トリガー: `on.pull_request.branches: [main]`, `on.push.branches: [main]`
- 環境変数: `CARGO_TERM_COLOR: always`, `RUSTFLAGS: "-D warnings"`
- ステップコマンド:
  - `cargo fmt -- --check`
  - `cargo clippy --all-targets`
  - `cargo test --lib -- --test-threads=1`
- ツールチェイン: Rust stable + rustfmt + clippy components

## 学び / トレードオフ
- `--test-threads=1` は SQLite 等の共有状態を使うユニットテストで必須。並列化による高速化は犠牲になるが、フレーキーテスト回避を優先。
- `RUSTFLAGS="-D warnings"` はワークフロー全体に影響するため、将来的に build 時の warnings 許容が必要になった場合は env のスコープ再設計が必要。
- `cargo test --lib` は `tests/` 配下の統合テストをカバーしないため、統合テストの CI 実行は後続仕様（`ci-integration-test-and-security-audit`）で追加される前提。
- Dependabot によるアクション自動更新はスコープ外。アクション破壊的変更のリスクは SHA 固定で緩和するにとどめる。
