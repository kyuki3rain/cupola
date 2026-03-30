# Research & Design Decisions

## Summary
- **Feature**: `release-workflow`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - GitHub Actions のクロスコンパイルは matrix strategy で各 OS ランナー上でネイティブビルドする標準パターン
  - softprops/action-gh-release は GitHub Release 作成のデファクトスタンダード Action
  - 全 Action はコミットハッシュでピン留めすることでサプライチェーン攻撃リスクを低減

## Research Log

### GitHub Actions クロスコンパイル戦略
- **Context**: 3 ターゲット（x86_64-unknown-linux-gnu, aarch64-apple-darwin, x86_64-apple-darwin）のビルド方法
- **Sources Consulted**: Issue 本文の参考 YAML、GitHub Actions ドキュメント
- **Findings**:
  - ubuntu-latest で Linux ターゲット、macos-latest で macOS ターゲットをビルド
  - aarch64-apple-darwin は macos-latest（Apple Silicon）でネイティブビルド可能
  - x86_64-apple-darwin も macos-latest で cross-compile 可能（Rosetta 2 環境）
  - fail-fast: false で独立ビルドを保証
- **Implications**: 各ターゲットは独立した OS ランナーで並列実行。特別なクロスコンパイルツールは不要

### Action バージョンピン留め
- **Context**: サプライチェーンセキュリティのベストプラクティス
- **Sources Consulted**: GitHub Security Hardening guide
- **Findings**:
  - タグ参照（@v4）はミュータブルで改ざんリスクあり
  - コミット SHA 参照（@<40 桁の SHA-1>）はイミュータブルで安全
  - Issue 参考 YAML で既にハッシュピン留め済み
- **Implications**: 参考 YAML のハッシュ値をそのまま採用

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 単一ワークフロー（build + release） | build matrix と release ジョブを1ファイルに定義 | シンプル、管理容易 | ジョブ数が増えると可読性低下 | Issue 参考 YAML と同一パターン。現状3ターゲットなので十分 |

## Design Decisions

### Decision: ワークフロー構成
- **Context**: Release ワークフローの構成方法
- **Alternatives Considered**:
  1. 単一ワークフロー（build matrix + release ジョブ）
  2. 複数ワークフロー（build と release を分離、workflow_run でチェイン）
- **Selected Approach**: 単一ワークフロー
- **Rationale**: 3 ターゲットのシンプルな構成では単一ファイルで十分。Issue の参考 YAML もこのパターン
- **Trade-offs**: シンプルさを優先。ターゲット増加時はリファクタリングが必要になる可能性
- **Follow-up**: なし

### Decision: パッケージング形式
- **Context**: バイナリの配布形式
- **Alternatives Considered**:
  1. tar.gz（全プラットフォーム共通）
  2. tar.gz（Linux/macOS）+ zip（Windows）
- **Selected Approach**: tar.gz のみ
- **Rationale**: Windows ターゲットはスコープ外。Linux/macOS ユーザーには tar.gz が標準
- **Trade-offs**: Windows サポートは将来の拡張として残す
- **Follow-up**: なし

## Risks & Mitigations
- macos-latest ランナーのアーキテクチャ変更リスク — GitHub のランナー更新を定期的に確認
- Action のハッシュが古くなるリスク — Dependabot や Renovate で自動更新を検討（スコープ外）
