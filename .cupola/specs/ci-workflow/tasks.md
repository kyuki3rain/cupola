# Implementation Plan

- [ ] 1. CI ワークフローファイルの作成と基本構成
  - `.github/workflows/ci.yml` を新規作成し、ワークフロー名を `CI` に設定する
  - トリガーとして `pull_request`（main ブランチ向け）と `push`（main ブランチ）を設定する
  - ワークフローレベルの環境変数 `CARGO_TERM_COLOR: always` と `RUSTFLAGS: "-D warnings"` を定義する
  - 単一ジョブ `check`（`ubuntu-latest`）を定義し、以下のセットアップステップを追加する:
    - `actions/checkout` をコミットハッシュ固定で使用
    - `dtolnay/rust-toolchain@stable` で rustfmt, clippy コンポーネントを指定
    - `Swatinem/rust-cache` をコミットハッシュ固定で使用しビルドキャッシュを有効化
  - 品質チェックステップを逐次実行する形で追加する:
    - `cargo fmt -- --check` によるフォーマットチェック
    - `cargo clippy --all-targets` による静的解析
    - `cargo test --lib -- --test-threads=1` によるユニットテスト実行
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3, 4.1, 4.2, 5.1, 5.2, 6.1, 6.2_

- [ ] 2. CI ワークフローの動作検証
  - ワークフローファイルの YAML シンタックスが正しいことを確認する
  - 現在のコードベースで `cargo fmt -- --check`、`cargo clippy --all-targets`（RUSTFLAGS="-D warnings"）、`cargo test --lib -- --test-threads=1` がローカルで全てパスすることを確認する
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.3, 4.1, 4.2_
