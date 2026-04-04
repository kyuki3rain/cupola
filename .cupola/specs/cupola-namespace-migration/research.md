# Research & Design Decisions

## Summary
- **Feature**: `cupola-namespace-migration`
- **Discovery Scope**: Extension（既存システムの namespace 移行 + ライフサイクル拡張）
- **Key Findings**:
  - prompt.rs の design/impl prompt を skill 委譲に変更する際、`feature_name` 引数が `Option<String>` → `String` になるため、impl prompt の None 分岐が不要になる
  - compress サブコマンドは既存の `ClaudeCodeRunner` trait をそのまま活用して `/cupola:spec-compress` を実行できる
  - PR_CREATION_SCHEMA の `feature_name` フィールドは design prompt 側で不要になる（CLI が `issue-{number}` を既に知っている）

## Research Log

### prompt.rs の簡素化影響範囲
- **Context**: design prompt は現在4つの kiro コマンドを順次指示。impl prompt は feature_name の有無で分岐
- **Sources Consulted**: `src/application/prompt.rs`（全842行）
- **Findings**:
  - `build_design_prompt`: `/kiro:spec-init` → `/kiro:spec-requirements` → `/kiro:spec-design -y` → `/kiro:spec-tasks -y` の4ステップ指示を含む。`/cupola:spec-design issue-{number}` 1行に置換可能
  - `build_implementation_prompt`: `feature_name: Option<&str>` で None 分岐あり。`String` 化により None 分岐削除可能
  - `PR_CREATION_SCHEMA`: `feature_name` フィールドを含む。design 時は CLI が `issue-{number}` を管理するため、schema から削除可能
  - fixing prompt は kiro 参照なし、変更不要
- **Implications**: prompt.rs のテスト20件超が影響を受ける。特に `feature_name: Option` 前提のテストの書き換えが必要

### Spec ディレクトリ初期化のタイミング
- **Context**: 現在は kiro:spec-init（LLM）がディレクトリ作成。CLI に移管する
- **Sources Consulted**: `src/application/polling_use_case.rs:286-335`（initialize_issue メソッド）
- **Findings**:
  - `initialize_issue` は worktree 作成後に呼ばれる。ここに spec ディレクトリ作成を追加するのが自然
  - issue 本文は `.cupola/inputs/issue.md` に書き出し済み（design prompt が参照）。requirements.md の `PROJECT_DESCRIPTION` にも同じ内容を埋め込む
  - テンプレート `init.json` と `requirements-init.md` が `.cupola/settings/templates/specs/` にある。プレースホルダ置換は Rust 側で行う
- **Implications**: `FileGenerator` trait に spec 初期化メ��ッドを追加するか、polling_use_case 内で直接ファイル操作するかの選択。Clean Architecture 的には前者が望ましい

### compress サブコマンドの Claude Code 呼び出し方式
- **Context**: compress は LLM による要約が必要。既存の CLI サブコマンドで Claude Code を呼ぶ仕組みはない
- **Sources Consulted**: `src/adapter/outbound/claude_code_process.rs`、`src/bootstrap/app.rs`
- **Findings**:
  - `ClaudeCodeProcess::spawn()` は prompt + working_dir + json_schema + model を受け取る
  - 既存は polling loop からのみ呼ばれる。compress では CLI 直接呼び出しが必要
  - compress は worktree 不要（メインブランチで実行）、output schema 不要（対話的に要約）
  - `--dangerously-skip-permissions` は compress では不要かもしれない（ユーザーが手動実行するため）
- **Implications**: compress 用の prompt を `prompt.rs` に追加し、bootstrap で `ClaudeCodeProcess` を compress にも注入する。ただし��compress は `--dangerously-skip-permissions` なしで呼ぶか、skill 呼び出し方式を検討

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| skill 内完結 | spec-design.md 内で全ロジック記述 | 変更が skill ファイルだけで閉じる | prompt.rs の簡素化と連動必須 | 採用 |
| prompt.rs 内完結 | skill を使わず prompt.rs に全指示を埋め込む | Rust 側で完全制御 | prompt が肥大化、skill の再利用性なし | 不採用 |

## Design Decisions

### Decision: Spec ID を CLI 側で決定的に生成
- **Context**: LLM による feature name 生成は衝突リスクがあり、issue との紐付けが不明確
- **Alternatives Considered**:
  1. LLM 命名を維持し、衝突時にサフィックス追加
  2. `issue-{number}` で CLI が決定
- **Selected Approach**: Option 2 — `issue-{number}` 固定
- **Rationale**: ユニーク性が保証され、issue との紐付けが明示的。LLM の介入が不要
- **Trade-offs**: 人間が読みや���い名前にはならないが、issue 番号で十分追跡可能
- **Follow-up**: 既存 spec（LLM 命名の55件）は compress で段階的にアーカイブ

### Decision: PR_CREATION_SCHEMA から feature_name を削除
- **Context**: design 時に Claude Code が feature_name を出力していたが、CLI が `issue-{number}` を管理するため不要
- **Alternatives Considered**:
  1. schema に残して互換維持
  2. schema から削除し、CLI 側で `issue-{number}` を使用
- **Selected Approach**: Option 2 — schema から削除
- **Rationale**: feature_name は CLI が決定するため、LLM に出力させる必要がない
- **Trade-offs**: スキーマ変更により、既存の output パース処理の修正が必要
- **Follow-up**: `build_session_config` の `feature_name` パラメータも `String` 化

### Decision: compress は skill + CLI のハイブリッド
- **Context**: spec 圧縮は LLM 必要、log 圧縮は機械的（ただし log は別 issue）
- **Alternatives Considered**:
  1. 全て CLI 内で完結（要約も機械的に）
  2. 全て skill で実行
  3. CLI がオーケストレーター、LLM 部分は skill に委譲
- **Selected Approach**: Option 3 — CLI + skill ハイブリッド
- **Rationale**: CLI が `cupola compress` として呼ばれ、`/cupola:spec-compress` skill を Claude Code 経由で実行。責務が明確
- **Trade-offs**: compress 実行に Claude Code が必要（LLM コスト発生）
- **Follow-up**: log 圧縮���別 issue で CLI 単体処理として実装

## Risks & Mitigations
- テスト大量書き換え — prompt.rs の20件超のテストが影響。段階的に修正し、各ステップで cargo test を通す
- 既存 spec との互換 — LLM 命名の55件は compress で段階的にアーカイブ。移行期間中は両方の命名規則が共存
- compress の Claude Code 依存 — Claude Code がインストールされていない環境では compress が動作しない。doctor チェックに追加（別 issue）

## References
- [cc-sdd リポジトリ](https://github.com/gotalab/cc-sdd) — MIT ライセンスの skill プロンプト原典
- `.cupola/settings/rules/` — EARS format、design principles、tasks generation ルール
- `.cupola/settings/templates/specs/` — requirements、design、tasks、research テンプレート
