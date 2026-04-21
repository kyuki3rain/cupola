# Research Log: issue-346

## Summary

- **Feature**: Claude Code Permission 機構の活用 (`--dangerously-skip-permissions` 廃止 + テンプレート + マージ)
- **Discovery Scope**: Extension (既存システムへのセキュリティ強化)
- **Key Findings**:
  - `--dangerously-skip-permissions` は `claude_code_process.rs:179` に単一箇所で存在し、削除対象が明確
  - テンプレートは既存の `include_str!` 埋め込みパターンで統一的に実装可能
  - JSON マージは `serde_json::Value` の手動実装で十分 (外部ライブラリ不要)
  - `--output-format json` 使用時に permission denied がどのような JSON レスポンスになるかは実装時に確認が必要

## Research Log

### Claude Code の Permission Denied 挙動

- **Context**: `--dangerously-skip-permissions` 削除後、Claude Code が permission denied に当たったときの挙動を把握する必要がある
- **Sources Consulted**: Issue #346 本文、既存の `claude_code_process.rs` の実装
- **Findings**:
  - `--output-format json` フラグにより Claude Code は non-interactive モードで動作する
  - permission denied 時は対話プロンプトに入らず、JSON 形式のエラーレスポンスを返すと想定
  - 具体的なエラーレスポンスのフィールド名 (`type`, `message` 等) は実装時に Claude Code の実際の動作を確認する
- **Implications**: 既存の JSON パーサーに `permission_denied` ケースを追加し、適切なエラーメッセージとヒントを出力する

### 既存 Assets 埋め込みパターン

- **Context**: テンプレートファイルをどのように組み込むか
- **Sources Consulted**: `src/adapter/outbound/init_file_generator.rs` の `CLAUDE_CODE_ASSETS` 定数
- **Findings**:
  - `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/<path>"))` パターンを全ファイルで統一使用
  - compile-time 埋め込みにより、実行時ファイル改ざんのリスクがない
  - `&[(&str, &str)]` 形式 (パス, コンテンツ) の定数配列で管理
- **Implications**: 同パターンで `assets/claude-settings/` テンプレートを埋め込む。パスは参照用のキーとして使用

### JSON マージ戦略の選択

- **Context**: 既存 `.claude/settings.json` とのディープマージの実装方法
- **Sources Consulted**: Issue #346 本文、既存 Cargo.toml 依存関係
- **Findings**:
  - `json-patch` クレートは RFC 6902 形式で、今回の union マージには過剰
  - `serde_json::Value` の再帰的マージは40行程度で実装可能
  - 配列の union は `HashSet<String>` への変換・合算・再変換で重複排除できる
  - `serde_json` はすでに依存関係に含まれている (`Cargo.toml` に `serde_json = "1"`)
- **Implications**: 外部ライブラリ追加なしで実装可能。`deep_merge_json(base: Value, overlay: Value) -> Value` を domain または application 層のユーティリティ関数として実装

### `cupola init` の既存実装

- **Context**: `--template` オプション追加時の影響範囲を把握する
- **Sources Consulted**: `src/adapter/inbound/cli.rs`、`src/application/init_use_case.rs`
- **Findings**:
  - `Init` バリアントは現在 `agent: InitAgent` と `upgrade: bool` の2フィールド
  - `InitUseCase::run` は `upgrade: bool` を受け取り、`InitReport` を返す
  - `InitReport` に `settings_json_written: bool` フィールドを追加する必要がある
  - steering bootstrap は `init_use_case.rs` 内で直接 `claude -p /cupola:steering --dangerously-skip-permissions` を呼び出しており、そこからもフラグを削除する
- **Implications**: `run` メソッドシグネチャに `templates: &[String]` を追加する。`FileGenerator` トレイトに `write_claude_settings` メソッドを追加する

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|-----------|------|------|------------|------|
| domain に `ClaudeSettings` 値オブジェクト | permission 設定を型で表現 | 型安全、層間の明確な境界 | struct 追加のオーバーヘッド | Clean Architecture に整合 |
| `serde_json::Value` のみで処理 | JSON を Value のまま扱う | 実装が簡易 | 型安全性が低い | テンプレートのマージには十分 |

→ **選択**: `ClaudeSettings` 値オブジェクトを domain 層に追加し、application 層の `TemplateManager` で `serde_json::Value` のマージを実施。型安全性と実装容易性を両立する。

## Design Decisions

### Decision: テンプレートの compile-time 埋め込み

- **Context**: テンプレート JSON をどのタイミングで読み込むか
- **Alternatives Considered**:
  1. 実行時ファイル読み込み — `~/.cupola/assets/` 等から読む
  2. compile-time 埋め込み (`include_str!`) — バイナリにバンドル
- **Selected Approach**: compile-time 埋め込み
- **Rationale**: 既存の `CLAUDE_CODE_ASSETS` パターンと統一できる。実行時ファイル依存がなく、テンプレートの改ざんリスクがない
- **Trade-offs**: テンプレート変更にはバイナリの再ビルドが必要。ただし OSS の性質上これは許容範囲
- **Follow-up**: `assets/claude-settings/` ディレクトリを CARGO_MANIFEST_DIR 相対パスで参照する

### Decision: `TemplateManager` を application 層に配置

- **Context**: テンプレートロード・マージロジックの層
- **Alternatives Considered**:
  1. domain 層 — 純粋なマージロジック
  2. application 層 — use case ロジックの一部
  3. adapter 層 — ファイル生成処理の一部
- **Selected Approach**: application 層の `template_manager.rs` モジュール
- **Rationale**: テンプレートのロードと選択は use case ロジック。`InitUseCase` に直接書くと肥大化するため分離
- **Trade-offs**: 新ファイル追加が必要だが、責務が明確になる

### Decision: マージ方向 (overlay 順序)

- **Context**: 複数テンプレートのオーバーレイ順序と既存 settings.json との関係
- **Selected Approach**: `base` → 追加テンプレート (指定順) → 既存 settings.json
  - テンプレート間: `allow`/`deny` の union
  - 既存 settings.json との統合: `allow`/`deny` の union、スカラーは既存優先
- **Rationale**: ユーザーのカスタマイズを最優先にしつつ、管理テンプレートの権限を確実に追加する
- **Trade-offs**: deny リストを既存が上書きできるため、セキュリティ上のリスクがある。ただしユーザー自身が設定する場合は自己責任として許容し、SECURITY.md に明記する

## Risks & Mitigations

- **`--dangerously-skip-permissions` 削除後の自動化停止** — base.json の allow リストを網羅的に設定し、permission 不足時の明確なエラーログとヒントで対処
- **既存ユーザーの `.claude/settings.json` との競合** — deep merge により既存設定を union で保持し、スカラーは既存を優先
- **steering bootstrap の失敗** — 既存実装と同様に graceful failure として扱い、`InitReport.steering_bootstrap_message` にエラー内容を記録
- **テンプレートの permission 不足によるスタック固有コマンドのブロック** — 各スタックテンプレートを十分なカバレッジで設計し、CONTRIBUTING.md でコントリビュートを促す
- **`init_file_generator.rs` の変更による既存 init 機能の回帰** — 既存テストを維持したまま新機能を追加し、統合テストで全体フローを確認

## References

- Issue #346: `--dangerously-skip-permissions` 廃止の詳細要件
- `src/adapter/outbound/init_file_generator.rs`: 既存の assets 埋め込みパターン
- `src/adapter/outbound/claude_code_process.rs:179`: フラグの削除対象箇所
- `src/adapter/inbound/cli.rs:66-73`: `Init` サブコマンドの現状定義
- Issue #320 (hash 改変検知), #321 (env whitelist), #319 (token 型安全化): 多層防御関連 issue
