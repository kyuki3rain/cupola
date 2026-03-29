# Research & Design Decisions

## Summary
- **Feature**: `ci-workflow`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - GitHub Actions の標準的な Rust CI パターンに従い、既存の参考定義をそのまま活用可能
  - actions/checkout, dtolnay/rust-toolchain, Swatinem/rust-cache はすべて安定版で広く使用されている
  - 単一ジョブ構成で十分（fmt → clippy → test の逐次実行）

## Research Log

### GitHub Actions Rust CI ベストプラクティス
- **Context**: Rust プロジェクトの標準的な CI 構成を確認
- **Sources Consulted**: Issue 本文の参考 CI 定義、GitHub Actions 公式ドキュメント
- **Findings**:
  - `dtolnay/rust-toolchain@stable` で rustfmt, clippy コンポーネントを指定するのが標準パターン
  - `Swatinem/rust-cache` はビルドキャッシュの事実上の標準
  - `RUSTFLAGS="-D warnings"` はワークフロー全体の env で設定するのが一般的
- **Implications**: Issue の参考定義がベストプラクティスに沿っており、大きな変更は不要

### アクションのバージョン固定
- **Context**: サプライチェーン攻撃対策としてコミットハッシュ固定が推奨される
- **Findings**:
  - Issue の参考定義では `actions/checkout` と `Swatinem/rust-cache` がコミットハッシュで固定済み
  - `dtolnay/rust-toolchain@stable` はタグ指定だが、このアクションはリポジトリオーナーが信頼できるため許容範囲
- **Implications**: 参考定義のバージョン固定ポリシーをそのまま採用

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 単一ジョブ構成 | 1 つの `check` ジョブで fmt, clippy, test を逐次実行 | シンプル、キャッシュ共有が容易 | 並列実行による高速化不可 | 現状の規模では十分 |
| マルチジョブ構成 | fmt, clippy, test を別ジョブで並列実行 | 高速化、個別失敗の特定が容易 | キャッシュ共有が複雑、コスト増 | 将来規模拡大時に検討 |

## Design Decisions

### Decision: 単一ジョブ構成の採用
- **Context**: CI のジョブ構成を決定する必要がある
- **Alternatives Considered**:
  1. 単一ジョブ — fmt, clippy, test を 1 ジョブで逐次実行
  2. マルチジョブ — 各チェックを並列ジョブで実行
- **Selected Approach**: 単一ジョブ構成
- **Rationale**: プロジェクト規模が小さく、ビルド時間が短いため並列化のメリットが薄い。Issue の参考定義もこの構成
- **Trade-offs**: シンプルさと保守性を優先し、並列実行による高速化は見送り
- **Follow-up**: ビルド時間が長くなった場合にマルチジョブ化を検討

## Risks & Mitigations
- アクションの破壊的変更 — コミットハッシュ固定で軽減（dependabot による更新推奨）
- テストのフレーキー — `--test-threads=1` で並列実行起因の不安定性を回避

## References
- [GitHub Actions ドキュメント](https://docs.github.com/en/actions) — ワークフロー構文リファレンス
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) — Rust ツールチェイン設定アクション
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) — Rust ビルドキャッシュアクション
