# Design Document: issue-334

## Overview

本フィーチャーは `docs/architecture/` 配下の 2 ファイルに対する軽微なドキュメント修正を行う。設計ドキュメントと実装の差異として発見された 2 件のみを対象とし、実装コードの変更は一切含まない。

**Purpose**: ドキュメントと実装の乖離を解消し、開発者が各コンポーネントの責務を正確に把握できるようにする。

**Users**: cupola のコードベースを読む開発者。

**Impact**: `docs/architecture/metadata.md` および `docs/architecture/effects.md` の 2 ファイルのみ更新する。

### Goals

- `metadata.md` の `feature_name` テーブルを実装（Collect Discovery）に合わせて修正する
- `effects.md` の `SpawnInit` 処理内容に `state=running` を追記して `polling-loop.md` との一貫性を確保する

### Non-Goals

- 実装コードの変更
- 上記 2 件以外のドキュメント修正（他 18 Gap は精査済みで対応不要と判定済み）
- テストコードの変更

## Requirements Traceability

| Requirement | Summary | 対象ファイル |
|-------------|---------|-------------|
| 1.1, 1.2, 1.3, 1.4 | feature_name 初期化主体を Collect に修正 | `docs/architecture/metadata.md` |
| 2.1, 2.2, 2.3 | SpawnInit に state=running を追記 | `docs/architecture/effects.md` |

## Architecture

### Existing Architecture Analysis

本フィーチャーはドキュメント変更のみ。既存アーキテクチャへの影響なし。

### Architecture Pattern & Boundary Map

変更対象はすべて静的な Markdown ドキュメントであり、アーキテクチャ図は不要。

**変更対象ファイルの分類**:

| ファイル | 種別 | 変更内容 |
|---------|------|---------|
| `docs/architecture/metadata.md` | アーキテクチャドキュメント | feature_name テーブルのタイミング・主体を修正 |
| `docs/architecture/effects.md` | アーキテクチャドキュメント | SpawnInit 処理内容に `state=running` を追記 |

### Technology Stack

本フィーチャーはドキュメント変更のみであり、追加ライブラリ・ランタイムは不要。

| Layer | Choice | Role |
|-------|--------|------|
| ドキュメント | Markdown | 変更対象ファイル形式 |

## Components and Interfaces

### ドキュメント変更コンポーネント一覧

| Component | 種別 | Intent | Req Coverage |
|-----------|------|--------|--------------|
| metadata.md feature_name 修正 | アーキテクチャドキュメント | 初期化主体を Collect に修正 | 1.1, 1.2, 1.3, 1.4 |
| effects.md SpawnInit 追記 | アーキテクチャドキュメント | state=running を明記 | 2.1, 2.2, 2.3 |

### Documentation Layer

#### `docs/architecture/metadata.md` 修正

| Field | Detail |
|-------|--------|
| Intent | `feature_name` テーブルの誤った初期化タイミング・主体を実装に合わせて修正する |
| Requirements | 1.1, 1.2, 1.3, 1.4 |

**変更仕様**:

修正前（`metadata.md:30-34`）:

```markdown
### `feature_name`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `Idle → InitializeRunning` 遷移時（デフォルト: `issue-{N}`） | Persist（Decide が決定） |
```

修正後:

```markdown
### `feature_name`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | Collect の Discovery で新規 issue を DB 登録する時（デフォルト: `issue-{N}`） | Collect |
```

**実装ノート**:
- `observations.md:107` の「Discovery は Collect が例外的に DB 書き込みを行う箇所」との整合性を確認する
- ファイル先頭の「Collect は純粋な観測のみで DB を書かない。」という注記は Discovery の例外を説明していないため、注記との矛盾を避けるため変更後の記述でも「例外的」であることを読み取れる形にする（`observations.md:107` への参照は本 doc に不要だが矛盾がないことを確認する）

#### `docs/architecture/effects.md` 修正

| Field | Detail |
|-------|--------|
| Intent | `SpawnInit` の処理内容セルに `state=running` を追記して `polling-loop.md` との一貫性を確保する |
| Requirements | 2.1, 2.2, 2.3 |

**変更仕様**:

修正前（`effects.md:140`）:

```markdown
| **処理内容** | ProcessRun(type=init) INSERT → fetch / worktree 作成 / `cupola/{feature_name}/main` branch 作成・push / `cupola/{feature_name}/design` branch 作成・push / `.cupola/specs/{feature_name}/` 作成（`spec.json` + `requirements.md` 雛形生成）|
```

修正後:

```markdown
| **処理内容** | ProcessRun(type=init, state=running) INSERT → fetch / worktree 作成 / `cupola/{feature_name}/main` branch 作成・push / `cupola/{feature_name}/design` branch 作成・push / `.cupola/specs/{feature_name}/` 作成（`spec.json` + `requirements.md` 雛形生成）|
```

**実装ノート**:
- `type=init` の直後に `, state=running` を追記するのみ。他の記述は変更しない
- `polling-loop.md:164-179` の `ProcessRun { state: running, ... }` と一致することを確認する

## Testing Strategy

本フィーチャーはドキュメント変更のみであり、自動テストは不要。以下の目視確認を実施する。

### 手動確認項目

1. `docs/architecture/metadata.md` — `feature_name` テーブルのタイミングが「Collect の Discovery で新規 issue を DB 登録する時」になっているか
2. `docs/architecture/metadata.md` — `feature_name` テーブルの主体が「Collect」になっているか
3. `docs/architecture/effects.md` — `SpawnInit` 処理内容が `ProcessRun(type=init, state=running) INSERT` となっているか
4. `docs/architecture/effects.md` — Markdown テーブルのフォーマットが崩れていないか
5. 変更後のドキュメント間に新たな矛盾が生じていないか（`observations.md`、`polling-loop.md` との整合）

## Security Considerations

本フィーチャーはドキュメント修正のみであり、セキュリティ上の考慮事項はない。
