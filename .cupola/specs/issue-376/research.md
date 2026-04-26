# Research & Design Decisions

---
**Purpose**: PR #374 / #375 で実装された permission 機構のドキュメント整備のための調査記録。

---

## Summary
- **Feature**: permission 機構ドキュメント整備 (issue-376)
- **Discovery Scope**: Extension（実装済み機能のドキュメント追従）
- **Key Findings**:
  - `cupola init --template <key>` CLI オプションは既に実装済み（`src/adapter/inbound/cli.rs`）
  - `cupola.toml` の `[claude_code.permissions]` セクションは `ClaudeCodePermissionsConfig` として実装済み（`templates`, `extra_allow`, `extra_deny`）
  - 既存の外部ドキュメント（README.md, README.ja.md, SECURITY.md, CONTRIBUTING.md）に permission 機構への言及がない
  - 旧方式（`.claude/settings.json` 配布）への言及はアーカイブ済み spec（issue-346）内のみで、現行の外部ドキュメントには残っていない

## Research Log

### 実装済みの permission 機構の確認

- **Context**: ドキュメントに記載すべき実装の正確な仕様を把握するため
- **Sources Consulted**:
  - `src/adapter/inbound/cli.rs` — `--template` CLI オプション定義
  - `src/domain/claude_code_permissions_config.rs` — `ClaudeCodePermissionsConfig` 構造体
  - `src/application/template_manager.rs` — `TemplateManager` 実装と `TEMPLATES` 定数
  - `assets/claude-settings/*.json` — 組み込みテンプレートファイル
- **Findings**:
  - `cupola init --template <key>[,<key>]` でカンマ区切り複数指定が可能
  - 組み込みテンプレート: `base`, `rust`, `typescript`, `python`, `go`, `devbox`
  - `base` は常に暗黙に適用（明示指定も可だが二重適用されない）
  - `extra_allow` / `extra_deny` でテンプレート解決後の allow/deny リストをカスタマイズ可能
  - spawn 時に `--allowedTools` / `--disallowedTools` CLI フラグとして Claude Code に渡される
  - `.claude/settings.json` への書き込みは行わない
- **Implications**: ドキュメントには `base` が暗黙適用される動作を明示する必要がある

### 既存ドキュメントの現状調査

- **Context**: 更新対象ファイルの現状を把握するため
- **Sources Consulted**: `README.md`, `README.ja.md`, `SECURITY.md`, `CONTRIBUTING.md`
- **Findings**:
  - `README.md` の `cupola init` セクション: `--template` オプションへの言及なし
  - `README.md` の Configuration Reference: `[claude_code.permissions]` セクション記載なし
  - `README.ja.md`: README.md と同様の状況
  - `SECURITY.md`: Permission Model セクションなし、既存セクションは `Prompt Injection Risk`, `trusted_associations`, `Issue Body Approval and Tampering Detection`
  - `CONTRIBUTING.md`: permission template 追加手順なし
- **Implications**: 4 ファイルすべてに追記が必要。削除/書き換えが必要な旧方式言及は現行外部ドキュメントには存在しない

### テンプレートファイルの内容確認

- **Context**: ドキュメントのテンプレート一覧表に正確な説明を載せるため
- **Sources Consulted**: `assets/claude-settings/*.json`
- **Findings**:
  - `base.json`: git 操作（add/commit/checkout/branch/fetch/pull 等）、基本ファイル操作を allow。`Bash(rm -rf*)`, `curl`, `wget`, `ssh`, `Bash(git push*)`, `Bash(gh*)`, `WebFetch`, `WebSearch` を deny
  - `rust.json`: cargo サブコマンド（build/test/clippy/fmt/check/run/doc）, rustup を allow
  - `typescript.json`: npm/npx/node, TypeScript コンパイラ（tsc）を allow
  - `python.json`: python/pip/pytest/uv/ruff 等を allow
  - `go.json`: go サブコマンドを allow
  - `devbox.json`: `Bash(devbox*)` を allow
- **Implications**: テンプレート一覧表の「主な追加 allow 操作」列に具体的な内容を記載できる

## Design Decisions

### Decision: ドキュメント更新スコープ

- **Context**: どのファイルをどの範囲で更新するか
- **Alternatives Considered**:
  1. 新規ドキュメントファイルを作成（例: `docs/permissions.md`）して既存ファイルからリンク
  2. 既存ファイルに直接追記・更新
- **Selected Approach**: 既存ファイル（SECURITY.md, README.md, README.ja.md, CONTRIBUTING.md）への直接追記・更新
- **Rationale**: 新規ユーザーが最初に参照するファイルに情報を置くことで発見性が高い。README は既にセキュリティセクションへのリンクを持っており、SECURITY.md との役割分担が明確
- **Trade-offs**: 既存ファイルが大きくなるが、permission 機構は Cupola の主要機能であり README への記載は適切

### Decision: README.ja.md の更新範囲

- **Context**: README.md と README.ja.md は独立したファイルで同期が必要
- **Selected Approach**: README.ja.md に README.md と同等の変更を適用し、日本語で記述する
- **Rationale**: 両ファイルは現状でも並行してメンテナンスされており、一方だけ更新すると乖離が生じる

## Risks & Mitigations

- テンプレート一覧表の内容と実装（`assets/claude-settings/*.json`）の乖離 — 実装ファイルを直接参照して記述し、実装変更時は両者を更新するよう CONTRIBUTING.md に明記する
- `README.md` の目次更新漏れ — permission 関連セクション追加時に目次も同時更新する
