# cupola-readme

## Feature
OSS 公開に向けてリポジトリルートに `README.md` を新規作成し、初訪問者が「プロジェクト概要 → 前提条件 → セットアップ → 日常ワークフロー → CLI/設定リファレンス → アーキテクチャ → ライセンス」の順で再現可能な形で情報へ到達できる単一ドキュメントを整備する。

## 要件サマリ
- **プロジェクト概要**: 3 文以内。Cupola が「GitHub Issue を起点に設計・実装を自動化するローカル常駐エージェント」であることを明記。人間（Issue 作成・ラベル付与・PR レビュー）と自動化範囲（設計ドキュメント生成〜実装〜レビュー対応）を区別。
- **前提条件**: Rust stable、Claude Code CLI、gh CLI、Git、devbox を一覧。cc-sdd と Cupola の関係を 1-2 文で説明。devbox 一括セットアップを案内。
- **インストール・セットアップ**: clone → devbox shell → `cargo build` → `.cupola/cupola.toml` 作成 → `cupola init` → `agent:ready` ラベル作成 → `cupola run` の順。番号付き再現可能手順。
- **使い方**: Issue 作成 → `agent:ready` 付与 → Cupola 設計生成 → 設計 PR → 人間レビュー → 実装生成 → 実装 PR → 人間レビュー → merge → Cupola cleanup の 2 段階レビューフロー。人間/Cupola の操作を明確に区別。
- **CLI リファレンス**: `run`（`--polling-interval-secs` / `--log-level` / `--config`）、`init`、`status` の全サブコマンドを実行例付きで記述。
- **設定リファレンス**: `owner` / `repo` / `default_branch` / `language` / `polling_interval_secs` / `max_retries` / `stall_timeout_secs` / `[log] level` / `[log] dir` の全 9 項目を型・デフォルト値・説明付きのテーブルで提示。完全な `cupola.toml` 例を掲載。
- **アーキテクチャ概要**: Clean Architecture 4 レイヤー（domain / application / adapter / bootstrap）の責務と依存方向（内向きのみ）、`src/` 配下のディレクトリ構造を 2 階層まで提示。
- **ライセンス**: LICENSE 未作成のためプレースホルダー。作成後にリンク追加する旨を注記。

## アーキテクチャ決定
- **構成順序**: 「チュートリアル型」（概要 → セットアップ → 使い方 → リファレンス）vs 「リファレンス先行型」を検討。チュートリアル型を採用。初訪問者が「何ができるか → どうセットアップするか → どう使うか」の順で情報を求めるため。既存ユーザーのリファレンス到達は目次内部リンクで補完。
- **単一ファイル方針**: `docs/` への分割ではなく `README.md` 単体に集約。Issue 要求に合致し、現時点でのドキュメント量が単一ファイルで管理可能。将来肥大化時の分割可能性を認めつつ、初期コストを最小化。
- **情報源の優先順位**: CLI コマンドは `src/adapter/inbound/cli.rs`、設定項目は `src/domain/config.rs` の `Config` 構造体、ワークフローは `.cupola/steering/product.md`、アーキテクチャは `.cupola/steering/structure.md` を「信頼できる情報源」として参照。ソースコード変更時には README も追従する運用前提。
- **LICENSE プレースホルダー**: LICENSE ファイル未作成のため README 内では仮記述とし、作成後にリンクを追加する方針。LICENSE 作成自体はスコープ外。
- **人間/Cupola の視覚的区別**: 使い方セクションで絵文字やラベル付きリスト形式を採用し、各ステップが「人間の操作」か「Cupola の自動処理」かを一目で分かるように設計。
- **Non-Goals**: 詳細な API（rustdoc レベル）、CONTRIBUTING.md、チュートリアル形式のウォークスルー、多言語化はいずれもスコープ外として明示。

## コンポーネント
| Component | Domain | Intent |
|-----------|--------|--------|
| `README.md`（リポジトリルート） | ドキュメント | 8 論理セクションで構成される単一ドキュメント |
| OverviewSection | ドキュメント | 3 文以内のプロジェクト概要 |
| PrerequisitesSection | ドキュメント | 必須ツールと cc-sdd 関係の説明 |
| InstallSection | ドキュメント | セットアップ手順 |
| UsageSection | ドキュメント | Issue → merge の全ワークフロー |
| CLIRefSection | ドキュメント | 全サブコマンドのリファレンス |
| ConfigRefSection | ドキュメント | `cupola.toml` 全項目のテーブル |
| ArchSection | ドキュメント | Clean Architecture 概要 |
| LicenseSection | ドキュメント | ライセンス情報 |

## 主要インターフェース
- 目次: 8 セクションへの内部リンク
- 前提条件テーブル: ツール / 用途 / 備考
- 設定項目テーブル: 項目 / 型 / デフォルト値 / 説明
- コードブロック: 各 CLI 実行例、`cupola.toml` 完全例、ディレクトリツリー
- 形式: GitHub Flavored Markdown（GitHub 上で自動レンダリング）

## 学び / トレードオフ
- ソースコードを信頼できる情報源とする方針は、コード変更時の README 同期漏れリスクを常に抱える。将来的には doc-comment から自動生成するツール（例: clap-markdown）導入の余地あり。
- 単一 README は現時点の情報量には適するが、将来トラブルシューティング・FAQ・コントリビューションガイドなどが増えた場合は `docs/` への分離が必要になる。
- LICENSE プレースホルダーは OSS 公開前の段階として許容する一方、実際の公開までには必ず解消する必要がある。
- Clean Architecture の依存方向の説明は、コントリビューターが変更箇所を特定するための重要な入口情報として位置付けた。
- 「人間の操作 vs Cupola の自動処理」の視覚的区別は、Cupola の価値提案（人間の介入箇所を最小化）を一目で伝える効果がある。
