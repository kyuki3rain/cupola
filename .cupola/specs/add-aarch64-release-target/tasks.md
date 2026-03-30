# Implementation Plan

- [ ] 1. Release ワークフローの基本構造を作成する
- [ ] 1.1 ワークフローファイルを作成し、タグ push トリガーと matrix 設定を定義する
  - `.github/workflows/release.yml` を新規作成する
  - `on.push.tags` に `v*.*.*` パターンを設定してタグ push のみをトリガーとする
  - `strategy.matrix.include` で `x86_64-unknown-linux-gnu` と `aarch64-unknown-linux-gnu` の 2 ターゲットを定義する
  - 各 matrix エントリに `target`、`archive`、`apt_packages`（aarch64 のみ）、`linker`（aarch64 のみ）フィールドを設定する
  - `permissions: contents: write` を設定して Release へのアップロード権限を付与する
  - _Requirements: 1.1, 1.2_

- [ ] 2. ビルドステップを実装する
- [ ] 2.1 (P) Rust ツールチェーンのセットアップとキャッシュを設定する
  - `actions/checkout` でソースをチェックアウトする
  - `dtolnay/rust-toolchain@stable` に `targets: ${{ matrix.target }}` を渡してターゲットを追加する
  - `Swatinem/rust-cache@v2` を設定してビルドキャッシュを有効化する
  - _Requirements: 2.1_

- [ ] 2.2 (P) aarch64 向けクロスコンパイラのインストールを設定する
  - `if: matrix.apt_packages` の条件ステップで `sudo apt-get install -y ${{ matrix.apt_packages }}` を実行する
  - これにより x86_64 ビルドにはインストールステップが不要となる
  - _Requirements: 2.2_

- [ ] 2.3 リリースビルドを実行するステップを実装する
  - `cargo build --release --target ${{ matrix.target }}` を実行する
  - `env` ブロックで `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: ${{ matrix.linker || '' }}` を設定する（x86_64 では空文字列のため影響なし）
  - ビルド失敗時は GitHub Actions のデフォルト動作でジョブが失敗し、後続ステップがスキップされることを確認する（タスク 3 への依存あり）
  - _Requirements: 2.3, 2.4_

- [ ] 3. アーカイブ作成とリリースアップロードを実装する
- [ ] 3.1 バイナリを tar.gz アーカイブにパッケージングするステップを実装する
  - `target/${{ matrix.target }}/release/` ディレクトリに移動して `cupola` バイナリのみを `${{ matrix.archive }}` にアーカイブする
  - アーカイブをワークスペースルートに移動して後続ステップから参照できるようにする
  - _Requirements: 1.3, 3.2_

- [ ] 3.2 GitHub Release へのアップロードステップを実装する
  - `softprops/action-gh-release@v2` を使用してアーティファクトをアップロードする
  - `files: ${{ matrix.archive }}` を設定して各 matrix ジョブが対応するアーカイブをアップロードする
  - GitHub Release が存在しない場合は自動作成されることを確認する
  - _Requirements: 1.4, 3.1, 3.3, 3.4_

- [ ] 4. ワークフロー動作の検証
  - テストタグ（`v0.0.1-test` 等）を push してワークフロー全体が正常に動作することを確認する
  - GitHub Release に `cupola-x86_64-unknown-linux-gnu.tar.gz` および `cupola-aarch64-unknown-linux-gnu.tar.gz` の両方が含まれることを確認する
  - 既存 `ci.yml` がタグ push で誤ってトリガーされないことを確認する
  - _Requirements: 1.1, 1.2, 1.4, 2.1, 2.2, 2.3, 3.1, 3.3, 3.4_
