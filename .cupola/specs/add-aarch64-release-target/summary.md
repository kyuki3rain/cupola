# add-aarch64-release-target

## Feature
`v*.*.*` タグ push をトリガーに、`x86_64-unknown-linux-gnu` と `aarch64-unknown-linux-gnu` 向けバイナリを GitHub Actions でビルドし、GitHub Release へ自動公開する。Raspberry Pi 5 等 ARM64 Linux デバイス向け配布の需要に応える。

## 要件サマリ
- `.github/workflows/release.yml` を新設し、タグ push で matrix ビルドを実行。
- x86_64 / aarch64 の 2 ターゲットを少なくともサポート。
- 各ターゲットを `cupola-<target>.tar.gz` でアーカイブし、Release に自動アップロード（Release 未作成時は自動作成）。
- rusqlite `bundled` feature を含むクロスコンパイルが ubuntu-latest 上で成功すること。
- 既存 `ci.yml` の x86_64 ビルド・公開機能に影響を与えないこと。

## アーキテクチャ決定
- **クロスコンパイル手法**: `cross` crate ではなく `cargo + rustup target + gcc-aarch64-linux-gnu` を採用。ターゲットが 1 種のみで Docker を要する `cross` はオーバーキル。apt パッケージ 1 つで完結し ci.yml との整合性を保てる。ターゲット増加時には `cross` 移行を検討する。
- **ターゲット設定方式**: `matrix.include` で `target` / `archive` / `apt_packages` / `linker` をエントリ毎に定義し、`if: matrix.apt_packages` で x86_64 のインストール処理をスキップ。ジョブ分離より DRY で拡張容易。
- **Release アップロード**: `softprops/action-gh-release@v2` を各 matrix ジョブ内で直接呼び出す。並行実行しても同一 Release にアペンドされるため、作成ジョブ分離（2 段構成）より簡潔。
- **QEMU ネイティブビルド**は実行速度の問題で不採用。
- **セキュリティ方針**: 外部 Action は可能な限りコミットハッシュでピン留め。`dtolnay/rust-toolchain@stable` は上流推奨に従い例外。`permissions: contents: write` を明示。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `.github/workflows/release.yml` | CI/CD | タグ push をトリガーとした matrix リリースビルドと GitHub Release 公開 |
| `matrix.include` | CI/CD | ターゲット毎のビルドパラメータ一元管理 |
| Archive ステップ | CI/CD | `cupola` バイナリを tar.gz にパッケージング |
| `softprops/action-gh-release@v2` | CI/CD | Release 作成とアーティファクトアップロード |

## 主要インターフェース
- トリガー: `on.push.tags: ['v*.*.*']`
- matrix エントリ例:
  - `x86_64-unknown-linux-gnu`: apt_packages なし、linker なし
  - `aarch64-unknown-linux-gnu`: apt_packages=`gcc-aarch64-linux-gnu`, linker=`aarch64-linux-gnu-gcc`
- 環境変数: `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: ${{ matrix.linker || '' }}`（x86_64 では空文字列で無害化）
- ビルドコマンド: `cargo build --release --target ${{ matrix.target }}`
- アーカイブ: `tar -czf cupola-<target>.tar.gz cupola`
- アップロード: `softprops/action-gh-release@v2` with `files: ${{ matrix.archive }}`

## 学び / トレードオフ
- rusqlite bundled は `cc` crate 経由で C コンパイラを呼ぶため、単なる `rustup target add` だけでは不足で、クロスリンカー環境変数と `gcc-aarch64-linux-gnu` のインストールが必須となる。必要に応じ `CC_aarch64_unknown_linux_gnu` の明示も有効。
- `softprops/action-gh-release` の並行実行による既存 Release へのアペンド動作に依存しているため、Action の挙動変更に注意。バージョンピン留めで緩和。
- `gcc-aarch64-linux-gnu` の apt パッケージ名は Ubuntu メジャー更新で変わる可能性があり、ubuntu-latest のメジャー版切替時に要確認。
- 現時点で macOS/Windows ターゲット、Release Note 自動生成、`cross` 採用はスコープ外。将来ターゲット数が増えた場合は `cross` への乗り換えを検討する。
