# Research & Design Decisions

---
**Purpose**: 技術設計に影響する調査結果とアーキテクチャ上の判断を記録する。

---

## Summary

- **Feature**: `issue-334`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - `metadata.md` の `feature_name` セクションは `Idle → InitializeRunning` 遷移時に Persist/Decide が初期化すると記述しているが、実装は `collect.rs:74` の Discovery フェーズで Collect が `Issue::new(issue_number, format!("issue-{issue_number}"))` を呼んで初期化している
  - `effects.md` の `SpawnInit` 処理内容は `ProcessRun(type=init) INSERT` と記述しており、`state=running` が省略されているが、`polling-loop.md:164-179` では `let run = ProcessRun { state: running, ... }` と明示されている

## Research Log

### feature_name 初期化箇所の確認

- **Context**: `metadata.md:30-34` の記述と実装の乖離を確認
- **Sources Consulted**: `docs/architecture/metadata.md`、`src/application/polling/collect.rs:74`、`docs/architecture/observations.md:107`
- **Findings**:
  - `metadata.md` は「`Idle → InitializeRunning` 遷移時（デフォルト: `issue-{N}`）」「主体: Persist（Decide が決定）」と記述
  - 実装 `collect.rs:74`: `let issue = Issue::new(issue_number, format!("issue-{issue_number}"));` — Collect の Discovery で初期化
  - `docs/architecture/observations.md:107` は「Discovery は Collect が例外的に DB 書き込みを行う箇所」と説明しており Collect 主体であることと整合
- **Implications**: doc が実装と乖離。修正方針はドキュメントを実装に合わせる。コード変更不要

### SpawnInit の state=running 記載の確認

- **Context**: `effects.md:134-143` の `SpawnInit` 処理内容に `state` が明記されていない点を確認
- **Sources Consulted**: `docs/architecture/effects.md:134-143`、`docs/architecture/polling-loop.md:164-179`、`docs/architecture/metadata.md`（ProcessRun.state テーブル）
- **Findings**:
  - `effects.md:140`: `ProcessRun(type=init) INSERT` — `state` 指定なし
  - `polling-loop.md:164-179`: SpawnInit/SpawnProcess 実行時に Execute がプロセスをスポーン直前に `ProcessRun { state: running, ... }` を INSERT すると明記
  - `metadata.md:95-98`: ProcessRun レコード全体テーブルでは `INSERT（state=running）` と記述されているが SpawnInit 専用のドキュメントには反映されていない
- **Implications**: 軽微な追記のみ。`state=running` を明記することで `polling-loop.md` との一貫性が確保できる

## Architecture Pattern Evaluation

本フィーチャーはすべてドキュメントの修正であり、アーキテクチャ上の選択肢評価は不要。

## Design Decisions

### Decision: 単一 PR での一括修正

- **Context**: 2 件の修正はいずれも軽微なドキュメント修正
- **Alternatives Considered**:
  1. 個別 PR — 変更ごとにレビューサイクルが発生
  2. 単一 PR — まとめてレビュー・マージ可能
- **Selected Approach**: 単一 PR で一括対応
- **Rationale**: 変更規模が小さく相互依存なし。Issue #334 も「小さな PR 1 本で修正する」と明示している
- **Trade-offs**: なし（変更ファイルが異なるため競合リスクも皆無）
- **Follow-up**: ドキュメント変更のみのため `cargo clippy` / `cargo test` への影響なし

## Risks & Mitigations

- Markdown テーブルのフォーマット崩れ — 修正後に目視でテーブル構造を確認する
- 他ドキュメントとの整合性 — `docs/architecture/observations.md:107` はすでに Collect の DB 書き込みを説明しており追加修正不要

## References

- Issue #334: metadata.md feature_name 記述修正 + effects.md SpawnInit state 追記
- `docs/architecture/metadata.md:30-34` — 修正対象: feature_name テーブル
- `docs/architecture/effects.md:134-143` — 修正対象: SpawnInit 処理内容
- `docs/architecture/polling-loop.md:164-179` — 参考: SpawnInit の state=running 記述
- `docs/architecture/observations.md:107` — 参考: Collect の Discovery DB 書き込み説明
- `src/application/polling/collect.rs:74` — 参考: Issue::new による feature_name 初期化実装
