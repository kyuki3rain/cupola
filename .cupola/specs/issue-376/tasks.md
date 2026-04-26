# Implementation Plan

- [ ] 1. SECURITY.md に Permission Model セクションを追加する

- [ ] 1.1 Permission Model セクションの本文を作成する
  - `## Permission Model` 見出しを `## Reporting Security Vulnerabilities` セクションの直前に追加する
  - Cupola が spawn する Claude Code subprocess が `--dangerously-skip-permissions` を使わず、`--allowedTools` / `--disallowedTools` CLI フラグで permission を渡すことを説明する
  - `.claude/settings.json` への書き込みを行わないため対話 Claude Code セッションに影響しないことを明記する
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 1.2 組み込みテンプレート一覧表と設定例を追記する
  - 組み込みテンプレート一覧表（`base`, `rust`, `typescript`, `python`, `go`, `devbox` と主な追加 allow 操作）を追加する
  - `base` が常に暗黙適用されることを注記する
  - `[claude_code.permissions]` の設定例 TOML（`templates`, `extra_allow`, `extra_deny`）を追加する
  - `base` テンプレートの deny リスト（`Bash(gh*)`, `Bash(git push*)`, `WebFetch` 等）が Cupola 経由でのみ実行できる設計であることを説明する
  - _Requirements: 1.4, 1.5, 1.6_

- [ ] 2. README.md に permission 機構の説明を追記する

- [ ] 2.1 `cupola init` セクションに `--template` オプションを追記する
  - `### cupola init` のコマンド説明に `--template <key>[,<key>]` オプションの説明を追加する
  - Rust + devbox 環境および TypeScript 環境を例とした使用例コードブロックを追加する
  - `cupola.toml` の `[claude_code.permissions].templates` に保存されることを言及する
  - _Requirements: 2.1_

- [ ] 2.2 Configuration Reference に `[claude_code.permissions]` フィールドを追加する
  - Configuration Reference テーブルに `[claude_code.permissions] templates`, `extra_allow`, `extra_deny` フィールドを追加する（型・デフォルト値・説明付き）
  - `cupola.toml` 設定全体例に `[claude_code.permissions]` セクションのサンプルを追加する
  - テンプレート一覧表（`base`, `rust`, `typescript`, `python`, `go`, `devbox` と説明）を設定リファレンス内に追記する
  - _Requirements: 2.2, 2.3, 2.4_

- [ ] 2.3 目次を更新する
  - README.md の目次に permission 機構関連セクションへのリンクを追加する（必要に応じてセクション名を調整する）
  - _Requirements: 2.5_

- [ ] 3. README.ja.md に permission 機構の日本語説明を追記する

- [ ] 3.1 (P) `cupola init` セクションに `--template` オプションを日本語で追記する
  - README.md の 2.1 と同等の変更を日本語で適用する
  - 使用例コードブロックはそのまま使用し、説明文を日本語にする
  - _Requirements: 3.1_

- [ ] 3.2 (P) 設定リファレンスに `[claude_code.permissions]` フィールドを日本語で追加する
  - README.md の 2.2 と同等の変更を日本語で適用する
  - `cupola.toml` 設定全体例への `[claude_code.permissions]` 追加、テンプレート一覧表の日本語版を追加する
  - _Requirements: 3.2, 3.3, 3.4_

- [ ] 4. CONTRIBUTING.md に permission template 追加手順を追記する

- [ ] 4.1 新 permission template 追加セクションを作成する
  - `## Adding a New Permission Template` セクションを `## Coding Standards` セクションの後に追加する
  - `assets/claude-settings/<key>.json` ファイルの作成手順と最小構成 JSON 例を記載する
  - `deny` フィールドを追加する場合の例も記載する
  - _Requirements: 4.1_

- [ ] 4.2 TEMPLATES 配列への登録手順を記載する
  - `src/application/template_manager.rs` の `TEMPLATES` 定数への `("<key>", include_str!(...))` 追加方法をコードスニペット付きで説明する
  - _Requirements: 4.2_

- [ ] 4.3 テスト追加要件を記載する
  - 単独指定テスト（`build_settings(&["<key>"])` で期待する allow エントリを検証）の例を記載する
  - 他テンプレートとの merge テスト（`build_settings(&["<key>", "rust"])` 等）の例を記載する
  - `template_manager.rs` の `#[cfg(test)]` ブロックへの追加を指示する
  - _Requirements: 4.3_

- [ ] 4.4 README テンプレート一覧更新と命名規則を記載する
  - README.md および README.ja.md のテンプレート一覧表への追記を手順に含める
  - テンプレートキーの命名規則（言語系: `rust`, `python` / エコシステム系: `devbox`, `docker` / フレームワーク系: `nextjs`）を説明する
  - _Requirements: 4.4, 4.5_
