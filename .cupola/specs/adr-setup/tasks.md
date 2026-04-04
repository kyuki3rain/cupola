# 実装計画

## タスク一覧

- [x] 1. `docs/adr/` ディレクトリと ADR テンプレートを作成する
  - `docs/` ディレクトリが存在しない場合は作成する
  - `docs/adr/` ディレクトリを作成する
  - `docs/adr/template.md` を作成し、以下の 6 セクションを含める: `# ADR-NNN: タイトル`、`## ステータス`（Accepted / Superseded / Deprecated の有効値を例示）、`## コンテキスト`、`## 決定`、`## 理由`、`## 却下した代替案`
  - 各セクションに記載内容のガイドコメントを記述する
  - ファイル命名規則（`NNN-kebab-case-title.md` 形式）と採番ルール（新規 ADR は次の連番から）をテンプレート冒頭に明記する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4_

- [x] 2. 初回 ADR（001-stall-detection-event-handling）を作成する
  - タスク 1 の完了後に実施する（`docs/adr/` ディレクトリが必要）
  - `docs/adr/001-stall-detection-event-handling.md` を作成し、`template.md` のフォーマットに準拠させる
  - タイトル: 「ADR-001: step5_stall_detection でイベントを即時生成しない理由」
  - ステータス: `Accepted`
  - コンテキスト: stall kill 後のイベント処理方式（即時 push vs. 次サイクル step3 委譲）の選択問題を記述する
  - 決定: 「stall kill 後のイベントは次サイクルの step3 で一元処理する」と記録する
  - 理由: 即時方式（方式A）では `ProcessFailed → DesignRunning → DesignRunning` 自己遷移が発生し、stale exit guard が無効化され、step3 で `ProcessFailed` が再発火して `retry_count` が二重インクリメントされるバグが生じることを説明する
  - 却下した代替案: 「step5 で `ProcessFailed` を即時 push する方式（方式A）」とその却下理由を記録する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [x] 3. (P) `AGENTS.md` に ADR 参照セクションを追加する
  - 既存の「Quality Check」セクションの内容・構造を維持する
  - ファイル末尾に新セクション「Design References」を追加する
  - `docs/adr/` へのリンクを含める
  - AIエージェントが設計・実装判断を行う際に ADR を参照すべきである旨を明記する
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 4. (P) `CLAUDE.md` に ADR への言及を追加する
  - 既存の「Project Context > Paths」セクションを特定する
  - 同セクションの `Steering` と `Specs` の記載に続く形で `ADR: docs/adr/` を追記する
  - 設計判断時に `docs/adr/` を参照すること、という説明を 1 文追加する
  - 既存セクションの構造・フォーマットを維持する
  - _Requirements: 5.1, 5.2, 5.3_
