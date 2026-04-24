# release-workflow サマリ

## Feature
`v*.*.*` タグ push をトリガーとした GitHub Actions Release workflow を追加し、Linux/macOS 向けバイナリのクロスコンパイル・tar.gz パッケージング・GitHub Release 作成を完全自動化する。

## 要件サマリ
- トリガー: `push.tags: ["v[0-9]*.[0-9]*.[0-9]*"]`（glob フィルタ）+ ジョブ内で `^v[0-9]+\.[0-9]+\.[0-9]+$` の正規表現で厳密検証。
- ターゲット 3 種を matrix でビルド: `x86_64-unknown-linux-gnu` (ubuntu-latest)、`aarch64-apple-darwin` / `x86_64-apple-darwin` (macos-latest)。
- `fail-fast: false`、Rust stable、ターゲットごとの `rust-cache`、`cargo build --release`。
- `cupola-{target}.tar.gz` でパッケージ、`if-no-files-found: error`、`upload-artifact`。
- `needs: build` で Release ジョブを後続実行、`softprops/action-gh-release` で全 tar.gz 添付 + リリースノート自動生成。
- `permissions: contents: write` のみ、全 Action はコミット SHA ピン留め。

## アーキテクチャ決定
- **単一ワークフロー（build matrix + release ジョブ）** (採用): 3 ターゲットというシンプルな構成では十分。`workflow_run` でのワークフロー分離はオーバーエンジニアリング。
- **ネイティブビルドで matrix する**: 各 OS ランナー上で対応ターゲットをネイティブコンパイル。特別な cross-compile ツール（cross 等）不要。`aarch64-apple-darwin` は macos-latest (Apple Silicon) でネイティブ、`x86_64-apple-darwin` も macos-latest で実行可能。
- **tar.gz のみ**: Windows ターゲットは非スコープのため zip は不要。
- **2 段階のタグ検証**: GitHub Actions の `on.push.tags` glob は厳密なセマンティックバージョニング表現を書けないため、glob で広めに受けた後ジョブ内正規表現で厳密判定する。
- **コミット SHA ピン留め必須**: `@v4` 等のタグ参照はミュータブルでサプライチェーン攻撃リスクがあるため禁止。
- **最小権限原則**: `contents: write` のみ。
- 非スコープ: Windows、バイナリ署名/公証、Homebrew/apt 配布、Cargo.toml バージョン自動更新。

## コンポーネント
- `.github/workflows/release.yml`（新規、単一ファイル）:
  - `WorkflowTrigger`: タグ push イベントフィルタ + ジョブ内正規表現検証ステップ。
  - `WorkflowPermissions`: `contents: write`。
  - `BuildMatrix` (build ジョブ): 3 ターゲット matrix、`dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache`、`actions/checkout`。
  - `PackageStep`: tar.gz 作成 + `actions/upload-artifact`（`if-no-files-found: error`）。
  - `ReleaseJob` (release ジョブ): `needs: build`、`actions/download-artifact`、`softprops/action-gh-release`（`generate_release_notes: true`）。
- 既存コードへの変更なし。

## 主要インターフェース
```yaml
on:
  push:
    tags: ["v[0-9]*.[0-9]*.[0-9]*"]
permissions:
  contents: write
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-apple-darwin
            os: macos-latest
  release:
    needs: build
```

## 学び / トレードオフ
- GitHub Actions の `on.push.tags` は glob 限定でセマンティックバージョニング厳密判定ができない制約に対し、2 段階検証（glob + ジョブ内正規表現）という明快なパターンで対応。
- ハッシュピン留めはメンテナンスコスト（Dependabot/Renovate 自動更新は本スコープ外）とセキュリティのトレードオフ。
- 新規ファイル追加のみで既存コードに一切触れないため、リスクは極小。
