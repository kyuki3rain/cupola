# Research & Design Decisions

---
**Purpose**: add-aarch64-release-target の設計調査および意思決定の記録

**Usage**:
- ディスカバリーフェーズの調査活動と成果を記録する
- `design.md` には詳細すぎる設計上のトレードオフを文書化する
- 将来の監査や再利用のための参照・証拠を提供する
---

## Summary

- **Feature**: `add-aarch64-release-target`
- **Discovery Scope**: Extension（既存 CI パイプラインへの追加）
- **Key Findings**:
  - 現状 `.github/workflows/release.yml` は存在しないため、新規作成が必要
  - rusqlite の `bundled` feature は SQLite を C ソースからコンパイルするため、クロスコンパイル時に `gcc-aarch64-linux-gnu` が必須
  - GitHub Actions の matrix build + `softprops/action-gh-release` の組み合わせが最もシンプルな実装パス

## Research Log

### GitHub Actions クロスコンパイル手法の調査

- **Context**: ubuntu-latest で `aarch64-unknown-linux-gnu` をビルドする方法の選定
- **Sources Consulted**: GitHub Actions 公式ドキュメント、rust-cross README、cross crate GitHub
- **Findings**:
  - `cross` crate: Docker コンテナ内でビルド。対応ターゲットが多いが Docker デーモンが必要
  - `cargo + rustup target`: ネイティブツールチェーン方式。`gcc-aarch64-linux-gnu` を apt でインストールするだけで動作
  - rusqlite bundled は `cc` crate 経由で C コンパイラを呼ぶため、`CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` 環境変数でクロスリンカーを指定する必要がある
- **Implications**: `cross` は設定が複雑になるため、`cargo + rustup target` + `gcc-aarch64-linux-gnu` を採用する

### リリースアーティファクトのアップロード手法

- **Context**: ビルド済みバイナリを GitHub Release にアップロードする Action の選定
- **Sources Consulted**: `softprops/action-gh-release`、`actions/upload-release-asset` ドキュメント
- **Findings**:
  - `actions/upload-release-asset`: 旧来の方法。Release 作成ステップを別途設ける必要がある
  - `softprops/action-gh-release`: 単一ステップで Release 作成 + ファイルアップロード。matrix ジョブから並行実行しても既存 Release にアペンドする
- **Implications**: `softprops/action-gh-release@v2` を採用することで workflow を簡潔に保てる

### 既存 CI ワークフローとの統合分析

- **Context**: 既存 `ci.yml` との重複排除・整合性確認
- **Sources Consulted**: `.github/workflows/ci.yml`
- **Findings**:
  - ci.yml は PR/main push に限定、release.yml は `v*.*.*` タグ push に限定するため干渉しない
  - ci.yml と同様に `dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache` を再利用できる
  - リリースビルドは `--release` フラグが必要（ci.yml はデバッグビルドのみ）
- **Implications**: release.yml は ci.yml から Action バージョンを踏襲し、独立して動作する

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|-----------|------|------|------------|------|
| cross crate | Docker コンテナ内クロスビルド | 多数ターゲット対応、環境汚染なし | Docker デーモン必要、実行時間が長い | 今回ターゲットが少ないためオーバーキル |
| cargo + apt クロスコンパイラ | `gcc-aarch64-linux-gnu` を apt インストール | シンプル、高速 | ターゲット増加時に設定が煩雑になる | **採用** |
| cargo + QEMU エミュレーション | ARM64 VM でネイティブビルド | クロスコンパイル不要 | 実行が極めて遅い（10×以上） | 不採用 |

## Design Decisions

### Decision: クロスコンパイル手法の選定

- **Context**: rusqlite bundled feature を含む Rust バイナリを ubuntu-latest 上で aarch64 向けにクロスコンパイルする必要がある
- **Alternatives Considered**:
  1. `cross` crate — Docker コンテナベースのクロスビルド
  2. `cargo + rustup target + gcc-aarch64-linux-gnu` — apt でクロスツールチェーンをインストール
- **Selected Approach**: `cargo + rustup target + gcc-aarch64-linux-gnu`
- **Rationale**: ターゲットが aarch64 1 種のみであり、`cross` の複雑さは不要。apt パッケージ 1 つで完結し、ci.yml との一貫性を保てる
- **Trade-offs**: cross に比べてターゲット追加時の汎用性は低いが、現時点では十分
- **Follow-up**: ターゲット数が増えた場合は cross への移行を検討する

### Decision: matrix include 方式によるターゲット設定

- **Context**: x86_64 と aarch64 で設定が異なる（apt パッケージ、リンカー環境変数）
- **Alternatives Considered**:
  1. matrix include でターゲットごとに全パラメータを定義
  2. 個別ジョブとして分離
- **Selected Approach**: `matrix.include` でターゲットごとに `apt_packages`、`linker` を定義し、`if: matrix.apt_packages` で条件実行
- **Rationale**: ジョブを分離するより DRY であり、ターゲット追加も容易
- **Trade-offs**: matrix 定義が複雑になるが、ジョブ数の増加を防げる
- **Follow-up**: なし

### Decision: softprops/action-gh-release によるリリース管理

- **Context**: matrix ビルドの各ジョブから GitHub Release にアーティファクトをアップロードする
- **Alternatives Considered**:
  1. `softprops/action-gh-release@v2` — Release 作成と upload を一括
  2. Release 作成ジョブ + `actions/upload-release-asset` — 2 段構成
- **Selected Approach**: `softprops/action-gh-release@v2` を各ビルドジョブ内で実行
- **Rationale**: 単一ステップで完結し、matrix ジョブから並行実行しても既存 Release にアペンドできる
- **Trade-offs**: Release Note のカスタマイズが `softprops/action-gh-release` の設定に依存する
- **Follow-up**: Release Note テンプレートが必要になった場合は `body_path` オプションで対応

## Risks & Mitigations

- `gcc-aarch64-linux-gnu` パッケージ名の変更 — Ubuntu バージョンアップ時に確認する
- `softprops/action-gh-release` の破壊的変更 — バージョンをピン留め（`@v2`）することで緩和
- rusqlite bundled の C コンパイル失敗 — `CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc` を明示することで対処

## References

- dtolnay/rust-toolchain: Rust ツールチェーンインストール Action
- Swatinem/rust-cache: Cargo キャッシュ Action
- softprops/action-gh-release: GitHub Release 作成・アップロード Action
- rusqlite bundled feature: SQLite ソースバンドルビルドの仕組み
