# Requirements Document

## Project Description (Input)
## 概要

PR #374 / #375 で Cupola の Claude Code permission 機構を再設計し、`.claude/settings.json` 配布方式から **`cupola.toml` + CLI フラグ (`--allowedTools` / `--disallowedTools`)** 方式に移行した。本 issue では以下のドキュメント整備を行う:

1. **SECURITY.md**: 新しい permission モデルの説明 (CLI フラグ経由、対話 Claude Code との分離)
2. **README.md / README.ja.md**: `cupola init --template <key>[,<key>]` の使い方、`[claude_code.permissions]` セクションの紹介
3. **CONTRIBUTING.md**: 新 permission template の追加方法 (`assets/claude-settings/{key}.json` + `template_manager::TEMPLATES` 追記 + テスト)

## 背景

元の #346 (closed) は `.claude/settings.json` 配布で permission を制御する設計だった。しかしこの方式は **対話 Claude Code セッションにも副作用** (project settings の deny が対話側にも適用される) があり、運用上の問題が判明した。

PR #374 の再設計:

- `cupola.toml` に `[claude_code.permissions].templates = ["rust", "devbox"]` を書く
- Cupola daemon が起動時に TemplateManager で permission を解決
- spawn 時に `--allowedTools "..."` / `--disallowedTools "..."` を CLI フラグで渡す
- `.claude/settings.json` は生成しない（対話 Claude Code への副作用ゼロ）

PR #375 で Cupola リポジトリ自身の `cupola.toml` にも `[claude_code.permissions] templates = ["rust", "devbox"]` を追加済み。

実装は完了しているが、**外部ドキュメント (README / SECURITY / CONTRIBUTING) が未追従**。新規ユーザーが「Cupola permission って何?」「どう設定するの?」を理解できる状態にする。

## 推奨対応

### 1. SECURITY.md 更新

既存の `Prompt Injection Risk` / `trusted_associations Feature` セクションに加えて、**Permission Model** セクションを追加:

- Cupola が spawn する Claude Code subprocess は `--dangerously-skip-permissions` を使わない
- `cupola.toml` の `[claude_code.permissions].templates` で許可する操作を制御
- 対話 Claude Code セッションとは独立 (project の `.claude/settings.json` には影響なし)
- 組み込みテンプレート: `base`, `rust`, `typescript`, `python`, `go`, `devbox`
- `--disallowedTools` で `Bash(gh*)`, `Bash(git push*)` 等を deny し、Cupola 経由でしか実行できない設計

### 2. README.md / README.ja.md 更新

`cupola init` の説明に `--template` オプションの使い方を追記:

```bash
# Rust プロジェクト + devbox 環境
cupola init --template rust,devbox

# TypeScript プロジェクト
cupola init --template typescript
```

`cupola.toml` の例セクションに `[claude_code.permissions]` を追加:

```toml
[claude_code.permissions]
templates = ["rust", "devbox"]
# extra_allow = ["Bash(my-tool*)"]
# extra_deny = ["Bash(forbidden-cmd)"]
```

組み込みテンプレート一覧表を併記。

### 3. CONTRIBUTING.md 更新

新 permission template を追加するための手順を明記:

1. `assets/claude-settings/<key>.json` を新規作成（最小は `{ "permissions": { "allow": ["Bash(<cmd>*)"] } }`）
2. `src/application/template_manager.rs` の `TEMPLATES` 配列に `("<key>", include_str!(...))` を追加
3. テストを 2 本追加（単独指定 / 他 template との merge）
4. README のテンプレート一覧に追記

命名規則: 言語 (`rust`, `python`)、エコシステム (`devbox`, `docker`)、フレームワーク (`nextjs`)。

## 受け入れ条件

- SECURITY.md に Permission Model セクションが追加される
- README.md / README.ja.md に `cupola init --template` の使い方と `[claude_code.permissions]` 設定例が記載される
- CONTRIBUTING.md に新 template 追加手順が記載される
- 既存 doc 内の `.claude/settings.json` 言及（旧方式）が、新方式に書き換え or 削除される

## 関連

- **#374** (実装本体): `cupola.toml` + CLI フラグ方式の実装
- **#375** (本リポジトリ設定): Cupola 自身の `cupola.toml` 設定追加
- **#347** (構想: permission OSS 化): 将来の独立パッケージ化構想

## 注記

本 issue は元 #346 の発展として位置付け。元 #346 は spec とアプローチが異なるため close 済み。


## Requirements

### Requirement 1: SECURITY.md への Permission Model セクション追加

**Objective:** 新規ユーザーとして、Cupola が Claude Code subprocess の権限をどのように制御しているかを理解したい。そのため SECURITY.md に permission モデルの説明を追加してほしい。

#### Acceptance Criteria

1.1. The SECURITY.md shall `## Permission Model` セクションを含み、Cupola が spawn する Claude Code subprocess が `--dangerously-skip-permissions` を使わないことを明記する

1.2. The SECURITY.md shall `cupola.toml` の `[claude_code.permissions].templates` フィールドで許可操作を制御できることを説明する

1.3. The SECURITY.md shall Cupola の permission 機構がプロジェクトの `.claude/settings.json` に書き込まず、対話 Claude Code セッションに影響を与えないことを明記する

1.4. The SECURITY.md shall 組み込みテンプレート一覧（`base`, `rust`, `typescript`, `python`, `go`, `devbox`）とその用途を説明する

1.5. The SECURITY.md shall `--disallowedTools` で `Bash(gh*)`, `Bash(git push*)` 等を deny することで Cupola の内部処理経由でのみ実行できる設計であることを説明する

1.6. The SECURITY.md shall `extra_allow` / `extra_deny` フィールドによるカスタマイズ方法を記載する

### Requirement 2: README.md への permission 機構説明追記

**Objective:** 英語ドキュメントを参照するユーザーとして、`cupola init` コマンドと `cupola.toml` の設定例から permission 機構を理解できるようにしたい。

#### Acceptance Criteria

2.1. When README.md の `cupola init` コマンド説明を読むとき、the ドキュメント shall `--template` オプションの用途と使い方（例: `--template rust,devbox`、`--template typescript`）を含む

2.2. The README.md shall Configuration Reference に `[claude_code.permissions]` セクションの説明（`templates`, `extra_allow`, `extra_deny` フィールド）を追加する

2.3. The README.md shall 組み込みテンプレート一覧表（`base`, `rust`, `typescript`, `python`, `go`, `devbox` とその説明）を含む

2.4. The README.md shall `cupola.toml` の設定全体例に `[claude_code.permissions]` セクションのサンプルを追加する

2.5. The README.md shall 目次に permission 機構に関するセクションへのリンクを追加する（またはセキュリティ関連セクションとの整合を確保する）

### Requirement 3: README.ja.md への permission 機構説明追記

**Objective:** 日本語ドキュメントを参照するユーザーとして、README.md と同等の permission 機構説明を日本語で参照できるようにしたい。

#### Acceptance Criteria

3.1. When README.ja.md の `cupola init` コマンド説明を読むとき、the ドキュメント shall `--template` オプションの用途と使い方の日本語説明を含む

3.2. The README.ja.md shall 設定リファレンスに `[claude_code.permissions]` セクションの日本語説明（`templates`, `extra_allow`, `extra_deny` フィールド）を追加する

3.3. The README.ja.md shall 組み込みテンプレート一覧表（日本語説明付き）を含む

3.4. The README.ja.md shall `cupola.toml` の設定全体例に `[claude_code.permissions]` セクションのサンプルを追加する

### Requirement 4: CONTRIBUTING.md への permission template 追加手順追記

**Objective:** Cupola にコントリビュートする開発者として、新しい permission template を正しく追加する手順を把握したい。

#### Acceptance Criteria

4.1. The CONTRIBUTING.md shall `assets/claude-settings/<key>.json` ファイルの作成手順と最小構成例（`{ "permissions": { "allow": ["Bash(<cmd>*)"] } }`）を含む

4.2. The CONTRIBUTING.md shall `src/application/template_manager.rs` の `TEMPLATES` 配列に `("<key>", include_str!(...))` を追加する方法を明記する

4.3. The CONTRIBUTING.md shall テスト追加の要件（単独指定テスト、他 template との merge テスト）を説明する

4.4. The CONTRIBUTING.md shall README へのテンプレート一覧追記を手順の一部として含む

4.5. The CONTRIBUTING.md shall テンプレートキーの命名規則（言語系: `rust`, `python` / エコシステム系: `devbox`, `docker` / フレームワーク系: `nextjs`）を説明する
