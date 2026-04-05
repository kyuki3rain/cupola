# Cupola アーキテクチャ概要

Cupola は GitHub Issue を監視し、Claude Code を自動的に起動して Design→Implementation の開発サイクルを回すエージェントオーケストレーターです。

## ドキュメント一覧

| ファイル | 内容 |
|---------|------|
| [state-machine.md](./state-machine.md) | 状態一覧・遷移ルール・イベント定義 |
| [polling-loop.md](./polling-loop.md) | ポーリングループの4フェーズ設計・副作用の扱い |
| [data-model.md](./data-model.md) | Issue・Config・メタデータのデータ構造 |

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

途中でレビューコメント・CI失敗・コンフリクトが発生した場合は Fixing 状態に遷移し、修正後に ReviewWaiting に戻ります。

## 設計原則

- **ポーリングループは4フェーズ**: Resolve（非同期完了回収）→ Collect（純粋観測）→ Apply（状態遷移）→ Execute（副作用実行）
- **Collect は副作用なし**: GitHub API・DB の読み取りのみ。DB 書き込みは Resolve / Apply / Execute が担当
- **完了は同サイクルで処理**: プロセス終了は Resolve フェーズで即時回収・後処理し、同サイクルの Collect・Apply に反映
- **リカバリを内蔵**: 再起動時も InitializeRunning から GitHub 状態を観測して正しい状態に復帰
