# Cupola アーキテクチャ概要

Cupola は GitHub Issue を監視し、Claude Code を自動的に起動して Design→Implementation の開発サイクルを回すエージェントオーケストレーターです。

## ドキュメント一覧

| ファイル | 内容 |
|---------|------|
| [state-machine.md](./state-machine.md) | 状態一覧・遷移ルール・遷移テーブル |
| [observations.md](./observations.md) | WorldSnapshot の定義・Collect のビルドロジック |
| [polling-loop.md](./polling-loop.md) | ポーリングループの5フェーズ設計 |
| [effects.md](./effects.md) | エフェクト一覧・実行モデル |
| [data-model.md](./data-model.md) | Issue・ProcessRun・Config のデータ構造 |
| [metadata.md](./metadata.md) | フィールド別の更新タイミングと更新主体 |

## 全体フロー

```
GitHub Issue (agent:ready ラベル)
  ↓ 検出
InitializeRunning  ← worktree・branch 作成
  ↓ 完了検知 + GitHub状態から遷移先を判断
DesignRunning      ← Claude Code が Design PR を作成
  ↓
DesignReviewWaiting
  ↓ Design PR マージ
ImplementationRunning  ← Claude Code が Impl PR を作成
  ↓
ImplementationReviewWaiting
  ↓ Impl PR マージ
Completed
```

途中でレビューコメント・CI失敗・コンフリクトが発生した場合は Fixing 状態に遷移し、修正後に ReviewWaiting に戻ります（CI失敗・コンフリクトは修正上限到達時を除く）。レビューコメントは trusted actor（`trusted_associations` によるロール判定 OR `trusted_reviewers` によるユーザー名判定）の unresolved スレッドのみ対象です。trusted なコメントを含まないスレッドは存在自体が秘匿されます。REQUEST_CHANGES 等スレッドを伴わないレビュー決定は観測されません（詳細は [observations.md](./observations.md) の `has_review_comments` 定義を参照）。

## 責務分担

Cupola と Claude Code では以下の通り責務を分けています。

| 処理 | 担当 |
|------|------|
| Git 操作（add / commit） | Claude Code |
| Git push / PR 作成 | Cupola |
| GitHub API 操作（label 操作、コメント投稿、Issue クローズ等） | Cupola |
| Design / Implementation 生成（`/cupola:spec-design` 等） | Claude Code |
| レビューコメント対応（`/cupola:fix`） | Claude Code |

**理由**: Claude Code は worktree 内でファイル編集と local commit を行い、push と PR 作成は Cupola が担当します。これにより、Claude Code には push 権限が不要（安全性の向上）かつ、PR 作成失敗時のリトライ制御も Cupola 側で一元管理できます。

## 設計原則

- **ポーリングループは5フェーズ**: Resolve（完了回収）→ Collect（純粋観測）→ Decide（純粋関数）→ Persist（DB コミット）→ Execute（副作用実行）
- **Collect は副作用なし**: GitHub API・DB の読み取りのみ。書き込みは Resolve / Persist / Execute が担当
- **Decide は DB 書き込みなし**: `(prev_state, WorldSnapshot) → (next_state, metadata_updates, effects)` を決定する純粋関数層
- **全エフェクトは Decide が決定する**: 一回性エフェクト（遷移検出）・持続性エフェクト（毎サイクル判定）ともに Decide で決定し、Execute が実行するだけ
- **観測値は状態スナップショット**: WorldSnapshot は「今この瞬間の世界の状態」であり、イベントではない。毎サイクル観測され続ける
- **リカバリを内蔵**: 再起動時も DB 上の状態を維持して継続。InitializeRunning 状態では GitHub・DB の現状を観測してスマートルーティングで正しい状態に復帰（DB に `pr_number` が記録されていることが前提。詳細は [observations.md](./observations.md) の既知制約を参照）
