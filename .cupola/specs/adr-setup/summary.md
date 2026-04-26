# adr-setup

## Feature
Cupola プロジェクトに ADR（Architecture Decision Records）基盤を整備し、設計判断を人間・AI エージェント双方が参照できる状態を確立する。`docs/adr/` ディレクトリ、テンプレート、初回 ADR（001）、および `AGENTS.md` / `CLAUDE.md` への参照追加を含む。

## 要件サマリ
- `docs/adr/` ディレクトリ作成とファイル命名規則 `NNN-kebab-case-title.md` の統一。
- `docs/adr/template.md` に 6 セクション（タイトル、ステータス、コンテキスト、決定、理由、却下した代替案）を含む標準テンプレートを用意。ステータス有効値は `Accepted` / `Superseded` / `Deprecated`。
- 初回 ADR `001-stall-detection-event-handling.md`（Accepted）で、`step5_stall_detection` で stall kill 後のイベントを即時 push しない決定を記録。
- `AGENTS.md` に Design References セクションを追加し、AI エージェントへ ADR 参照を誘導。
- `CLAUDE.md` の `Project Context > Paths` に `docs/adr/` を追記。

## アーキテクチャ決定
- **フォーマット**: Michael Nygard 式の簡略版（Issue 指定のフォーマット）を採用。業界標準で広く普及しており、将来参照性と独自拡張不要の観点で妥当。
- **配置**: `docs/adr/`（ルート直下 `adr/` ではなく）。GitHub 慣例に沿う。`docs/` は本フィーチャーで新規作成。
- **AGENTS.md 追記方針**: 既存 Quality Check セクションに混ぜず、新セクション「Design References」を追加。用途の違いを明確化し AI の参照精度を高める。
- **CLAUDE.md 追記方針**: 既存 `Paths` セクションに `ADR: docs/adr/` を 1 行追加。参照先集約場所として最自然。
- **空ディレクトリ対策**: `.gitkeep` 不要。`template.md` が最初のファイルとなる。

## コンポーネント
| Component | 種別 | Intent |
|-----------|------|--------|
| `docs/adr/` | ディレクトリ | ADR 格納場所 |
| `docs/adr/template.md` | ドキュメント | 新規 ADR コピー元テンプレート |
| `docs/adr/001-stall-detection-event-handling.md` | ドキュメント | 初回 ADR（step5 stall 処理決定記録） |
| `AGENTS.md`（更新） | ドキュメント | AI エージェントへの ADR 参照誘導 |
| `CLAUDE.md`（更新） | ドキュメント | Claude Code への ADR パス提示 |

## 主要インターフェース
- テンプレートセクション: `# ADR-NNN: タイトル` / `## ステータス` / `## コンテキスト` / `## 決定` / `## 理由` / `## 却下した代替案`
- 命名規則: `NNN-kebab-case-title.md`（連番 3 桁、以下 kebab-case）
- ADR-001 の記録内容: 決定「stall kill 後のイベントは次サイクルの step3 で一元処理する」。却下案「step5 で `ProcessFailed` を即時 push する方式（方式A）」—`ProcessFailed → DesignRunning → DesignRunning` 自己遷移で stale exit guard が無効化され step3 で `ProcessFailed` が再発火し `retry_count` 二重インクリメントを招くため。

## 学び / トレードオフ
- ドキュメント追加のみのためランタイムへの影響はゼロ。レビュー観点は「セクション欠落」「命名不正」「既存ファイル構造の破壊」。
- ADR は既存設計判断を遡及的に作成せず、001 からの前進のみを対象とする方針（工数と価値のバランス）。
- 自動生成・検索などの強化機能は将来課題として明示的にスコープ外。
- AGENTS.md を短く保ちつつセクション追加することで、AI エージェントが用途別に情報を引き出しやすくなる反面、ファイル長は増加する。
