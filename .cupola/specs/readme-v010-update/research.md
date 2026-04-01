# リサーチ・設計判断ログ

---
**目的**: 設計フェーズで行った調査・発見事項と判断根拠を記録する。

---

## Summary

- **Feature**: `readme-v010-update`
- **Discovery Scope**: Simple Addition（既存ドキュメントファイルの更新）
- **Key Findings**:
  - 本タスクはコード変更を伴わない純粋なドキュメント更新である
  - `cli.rs` の実装から、現行CLIコマンドは `start / stop / init / status / doctor / --version / -V` であることを確認
  - `src/` ディレクトリの実ファイル一覧を確認し、README記載のファイルツリーとの差分を特定した
  - `doctor_use_case.rs`, `init_use_case.rs`, `stop_use_case.rs`, `fixing_problem_kind.rs`, `check_result.rs` 等、README に未反映のファイルが複数存在する

---

## Research Log

### CLIコマンドの現状確認

- **Context**: Issue でコマンド名の変更（run → start）とコマンド追加が要求されている
- **Sources Consulted**: `src/adapter/inbound/cli.rs`
- **Findings**:
  - `Start` サブコマンド: `--polling-interval-secs`, `--log-level`, `--config`, `-d`/`--daemon` オプションを持つ
  - `Stop` サブコマンド: `--config` オプションを持つ
  - `Init` サブコマンド: オプションなし
  - `Status` サブコマンド: オプションなし
  - `Doctor` サブコマンド: `--config` オプションを持つ
  - `--version` / `-V` フラグ: `#[command(version)]` により自動生成
  - `cupola run` コマンドは存在しない（README に誤記）
- **Implications**: README の CLI Command Reference を完全に書き直す必要がある

### ファイルツリーの差分確認

- **Context**: Issue でファイルツリーを現在のコードと一致させることが要求されている
- **Sources Consulted**: `find src/ -type f -name "*.rs"` の実行結果
- **Findings**:
  現行READMEに記載されているが存在しないファイル: なし
  現行READMEに未記載だが存在するファイル:
  - `src/domain/check_result.rs`
  - `src/domain/fixing_problem_kind.rs`
  - `src/application/doctor_use_case.rs`
  - `src/application/init_use_case.rs`
  - `src/application/stop_use_case.rs`
  - `src/application/port/command_runner.rs`
  - `src/application/port/config_loader.rs`
  - `src/application/port/pid_file.rs`
  - `src/adapter/outbound/init_file_generator.rs`
  - `src/adapter/outbound/pid_file_manager.rs`
  - `src/adapter/outbound/process_command_runner.rs`
  - `src/bootstrap/toml_config_loader.rs`
  - `src/adapter/inbound/mod.rs`（mod.rsは省略可）
  - `src/adapter/mod.rs`（mod.rsは省略可）
  - `src/bootstrap/mod.rs`（mod.rsは省略可）
- **Implications**: ファイルツリーセクションを実ファイル構造で全面置換する

### 設定項目の確認

- **Context**: Issue で `max_concurrent_sessions` と `model` の追加が要求されている
- **Sources Consulted**: Issue 本文
- **Findings**:
  - `max_concurrent_sessions`: 同時実行数制限。デフォルトは無制限
  - `model`: デフォルト Claude モデル。デフォルトは `"sonnet"`
  - これらは既存の設定項目テーブルに追加行として記載する
- **Implications**: Configuration Reference テーブルと設定例コードブロックの両方を更新する

---

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| インプレース更新 | README.md を直接編集して全セクションを更新 | シンプル、ファイル数が増えない | 変更量が大きい | 今回採用 |
| セクション分割 | 各セクションを別ファイルに分けてインクルード | 再利用性が高い | GitHub の標準レンダリングでは不可 | 採用しない |

---

## Design Decisions

### Decision: `mod.rs はファイルツリーに含めない`

- **Context**: `find` 結果に `mod.rs` が含まれるが、これらは構造上の接合ファイルであり README の読み手にとって情報量が少ない
- **Alternatives Considered**:
  1. すべての `.rs` ファイルを列挙
  2. `mod.rs` を省略して実質的なロジックファイルのみ列挙
- **Selected Approach**: `mod.rs` は省略。ただし `main.rs` と `lib.rs` は含める
- **Rationale**: README のファイルツリーはアーキテクチャ理解を助けることが目的。`mod.rs` は構造的な接合点に過ぎず、読み手への価値が低い
- **Trade-offs**: 厳密な網羅性より可読性を優先する
- **Follow-up**: 実装時に実ファイルリストと最終調整すること

### Decision: `Limitations` セクションを独立した H2 セクションとして追加`

- **Context**: Issue で制限事項セクションの追加が要求されている
- **Alternatives Considered**:
  1. 既存セクション（FAQ、Usage 等）に注記として追加
  2. 独立した `## Limitations` セクションとして追加
- **Selected Approach**: 独立した `## Limitations` セクション
- **Rationale**: ユーザーが制限事項を明示的に参照できるよう、目次にリンクが現れることが望ましい
- **Trade-offs**: セクション数が増えるが、発見可能性が上がる

---

## Risks & Mitigations

- README.ja.md（日本語版）への反映漏れ — 本タスクのスコープ外（別Issueで対応すべき）。README.md の変更完了後に明示的に指摘すること
- ファイルツリーの陳腐化 — 将来のファイル追加時に手動更新が必要。CI での自動チェックが理想だが今回はスコープ外

## References

- Issue #74: README.md の全面更新（v0.1.0 リリース向け）
- `src/adapter/inbound/cli.rs` — CLI コマンド定義の根拠
