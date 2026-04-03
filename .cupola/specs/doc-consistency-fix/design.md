# Design Document: doc-consistency-fix

## Overview

本フィーチャーは v0.1 プレリリース監査で発見された4件のドキュメント・設定ファイルの不整合を修正する。対象はすべて軽微な変更であり、ソースコードの変更は一切含まない。

**Purpose**: ドキュメントと実装の乖離を解消し、利用者・開発者が正確な情報にアクセスできるようにする。

**Users**: cupola の利用者・開発者がこのドキュメント修正の恩恵を受ける。

**Impact**: `.cupola/steering/tech.md`、`CHANGELOG.md`、`README.md`、`README.ja.md`、`.gitignore` の5ファイルを更新する。

### Goals

- `steering/tech.md` のCLIサブコマンド名を実装と一致させる
- `CHANGELOG.md` に `start` / `stop` デーモン機能を記載する
- `README.md` / `README.ja.md` に Unix only 制約を明記する
- `.gitignore` に `.env` を追加してトークン漏洩リスクを排除する

### Non-Goals

- CLIサブコマンドの実装変更
- CHANGELOG 以外のリリースノート整備
- Windows 対応の実装

## Requirements Traceability

| Requirement | Summary | 対象ファイル |
|-------------|---------|-------------|
| 1.1, 1.2, 1.3 | steering/tech.md のサブコマンド名更新 | `.cupola/steering/tech.md` |
| 2.1, 2.2, 2.3 | CHANGELOG への start/stop 記載 | `CHANGELOG.md` |
| 3.1, 3.2, 3.3, 3.4 | README への Unix only 明記 | `README.md`, `README.ja.md` |
| 4.1, 4.2, 4.3 | .gitignore への .env 追加 | `.gitignore` |

## Architecture

### Existing Architecture Analysis

本フィーチャーはドキュメント変更のみ。既存アーキテクチャへの影響なし。

### Architecture Pattern & Boundary Map

変更対象はすべてリポジトリルートおよびプロジェクト設定ディレクトリ内の静的ファイルであり、アーキテクチャ図は不要。

**変更対象ファイルの分類**:

| ファイル | 種別 | 変更内容 |
|---------|------|---------|
| `.cupola/steering/tech.md` | プロジェクト設定 | CLIサブコマンド名・使用例の更新 |
| `CHANGELOG.md` | リリースノート | start/stop 機能の Added 記載 |
| `README.md` | ユーザー向けドキュメント | 要件セクションへの Unix only 明記 |
| `README.ja.md` | ユーザー向けドキュメント（日本語） | 同上（日本語） |
| `.gitignore` | Git 設定 | `.env` エントリ追加 |

### Technology Stack

本フィーチャーはドキュメント・設定変更のみであり、追加ライブラリ・ランタイムは不要。

| Layer | Choice | Role |
|-------|--------|------|
| ドキュメント | Markdown | 変更対象ファイル形式 |
| Git 設定 | .gitignore | セキュリティ設定 |

## Components and Interfaces

### ドキュメント変更コンポーネント一覧

| Component | 種別 | Intent | Req Coverage |
|-----------|------|--------|--------------|
| tech.md 更新 | steering doc | CLIサブコマンド名を実装と同期 | 1.1, 1.2, 1.3 |
| CHANGELOG.md 更新 | リリースノート | start/stop デーモン機能を記載 | 2.1, 2.2, 2.3 |
| README.md 更新 | ユーザードキュメント | Unix only 制約を明記 | 3.1, 3.3, 3.4 |
| README.ja.md 更新 | ユーザードキュメント（日本語） | Unix only 制約を明記（日本語） | 3.2, 3.3, 3.4 |
| .gitignore 更新 | Git 設定 | .env をトラッキング対象外に設定 | 4.1, 4.2, 4.3 |

### Documentation Layer

#### `.cupola/steering/tech.md` 更新

| Field | Detail |
|-------|--------|
| Intent | CLIサブコマンド一覧と使用例を最新実装に同期する |
| Requirements | 1.1, 1.2, 1.3 |

**変更仕様**:

- `Subcommands: run / init / status` → `start / stop / init / status / doctor`
- `cargo run -- run` → `cargo run -- start`

**実装ノート**:
- Validation: 変更後の記述が実装の `src/adapter/inbound/` に定義されたサブコマンドと一致することを目視確認する
- Risks: 他のドキュメントが `run` サブコマンドを参照している場合は同様に修正が必要（本フィーチャースコープ外の場合は別Issue化）

#### `CHANGELOG.md` 更新

| Field | Detail |
|-------|--------|
| Intent | v0.1 の Added セクションに start/stop デーモン機能を追記する |
| Requirements | 2.1, 2.2, 2.3 |

**変更仕様**:

v0.1.0（または `[Unreleased]`）の `### Added` セクションに以下を追記:

```
- `cupola start --daemon`: バックグラウンドデーモンとして起動するオプションを追加
- `cupola stop`: 実行中のデーモンを停止するサブコマンドを追加
```

**実装ノート**:
- Integration: 既存の CHANGELOG フォーマット（Keep a Changelog 形式）に準拠する
- Risks: v0.1.0 セクションが存在しない場合は `[Unreleased]` セクションへの追記で対応

#### `README.md` / `README.ja.md` 更新

| Field | Detail |
|-------|--------|
| Intent | 要件セクションに Unix (macOS / Linux) only の制約を明記する |
| Requirements | 3.1, 3.2, 3.3, 3.4 |

**変更仕様**:

- `README.md`: Requirements/Prerequisites セクションに「Unix (macOS / Linux) only」を追記
- `README.ja.md`: 同セクションに「Unix (macOS / Linux) のみ対応」を追記
- `nix` クレート (`cfg(unix)`) に起因する制約である旨をコメントとして添える

**実装ノート**:
- Validation: 要件セクションが存在しない場合は新規追加する
- Risks: README の構成が英語版と日本語版で異なる場合は対応するセクションを特定してから追記する

#### `.gitignore` 更新

| Field | Detail |
|-------|--------|
| Intent | .env ファイルを Git のトラッキング対象から除外する |
| Requirements | 4.1, 4.2, 4.3 |

**変更仕様**:

`.gitignore` に以下を追加:

```
.env
```

**実装ノート**:
- Security: `gh-token` クレートが `.env` からトークンを読み込む設計のため、誤コミットを防ぐ
- Validation: 既に `.env` または `*.env` が存在しないことを確認してから追加する
- Risks: 既存の `.env.example` 等は除外対象としない（パターンを限定的に `/.env` とすることも検討可）

## Testing Strategy

本フィーチャーはドキュメント・設定変更のみであり、自動テストは不要。以下の目視確認を実施する。

### 手動確認項目

1. `.cupola/steering/tech.md` — サブコマンド一覧が `start / stop / init / status / doctor` になっているか
2. `.cupola/steering/tech.md` — `cargo run -- start` の使用例が正しいか
3. `CHANGELOG.md` — start/stop コマンドの説明が Added セクションに存在するか
4. `README.md` — 要件セクションに Unix only の記述があるか
5. `README.ja.md` — 要件セクションに Unix only の記述（日本語）があるか
6. `.gitignore` — `.env` エントリが存在するか
7. `.gitignore` — 既存の `.env.example` 等が意図せず除外されていないか

## Security Considerations

- `.gitignore` への `.env` 追加は機密情報（GitHub トークン等）の誤コミット防止が目的
- 既存のトラッキング済み `.env` ファイルが存在する場合は `git rm --cached .env` が必要（本フィーチャースコープ外）
