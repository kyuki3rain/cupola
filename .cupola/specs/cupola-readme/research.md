# Research & Design Decisions

## Summary
- **Feature**: `cupola-readme`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - CLI は `run`（3 オプション）、`init`、`status` の 3 サブコマンドで構成される
  - `cupola.toml` は 7 トップレベル項目 + `[log]` セクション（2 項目）で構成される
  - LICENSE ファイルは未作成のため、README ではプレースホルダーとして記述する

## Research Log

### CLI コマンド構成の調査
- **Context**: README の CLI リファレンスセクションに正確な情報を記述するため
- **Sources Consulted**: `src/adapter/inbound/cli.rs`
- **Findings**:
  - `run`: `--polling-interval-secs`（u64, optional）、`--log-level`（String, optional）、`--config`（PathBuf, default: `.cupola/cupola.toml`）
  - `init`: オプションなし
  - `status`: オプションなし
- **Implications**: 各コマンドの実行例とオプション説明を正確に記述可能

### 設定ファイル構成の調査
- **Context**: README の設定リファレンスセクションに全項目を網羅するため
- **Sources Consulted**: `src/domain/config.rs`、`.cupola/cupola.toml`
- **Findings**:
  - トップレベル: `owner`（String）、`repo`（String）、`default_branch`（String）、`language`（String, default: "ja"）、`polling_interval_secs`（u64, default: 60）、`max_retries`（u32, default: 3）、`stall_timeout_secs`（u64, default: 1800）
  - `[log]` セクション: `level`（String, default: "info"）、`dir`（String, optional）
- **Implications**: 型情報とデフォルト値をソースコードから正確に記述可能

### ライセンスファイルの確認
- **Context**: README のライセンスセクションの記述方針を決定するため
- **Sources Consulted**: リポジトリルートの `LICENSE*` パターン検索
- **Findings**: LICENSE ファイルが存在しない
- **Implications**: README にはライセンス未定である旨を記述し、LICENSE ファイル作成後にリンクを追加する方針とする

## Design Decisions

### Decision: README の構成順序
- **Context**: 初訪問者が自然に読み進められるセクション配置が必要
- **Alternatives Considered**:
  1. リファレンス先行型 — CLI / 設定リファレンスを冒頭に配置
  2. チュートリアル型 — 概要→セットアップ→使い方→リファレンスの順
- **Selected Approach**: チュートリアル型
- **Rationale**: 初訪問者は「何ができるか」→「どうセットアップするか」→「どう使うか」の順で情報を求める。リファレンスは参照用途であり後半が適切
- **Trade-offs**: 既存ユーザーがリファレンスに到達するまでスクロールが必要だが、目次リンクで解決可能

### Decision: 単一 README ファイル構成
- **Context**: ドキュメントを単一ファイルにまとめるか、複数ファイルに分割するか
- **Alternatives Considered**:
  1. 複数ファイル — docs/ ディレクトリに分割
  2. 単一ファイル — README.md に全情報を集約
- **Selected Approach**: 単一ファイル
- **Rationale**: Issue の要求が README.md 単体であり、現時点ではドキュメント量が単一ファイルで管理可能な範囲。目次による内部リンクで可読性を確保する
- **Trade-offs**: 将来的にドキュメントが増えた場合は分割が必要になる可能性がある

## Risks & Mitigations
- LICENSE ファイル未作成 — README にプレースホルダーを配置し、作成後に更新する旨を記述
- cupola.toml の設定項目が将来変更される可能性 — ソースコードの Config 構造体を信頼できる情報源として記述し、変更時に README も更新するフローを推奨
