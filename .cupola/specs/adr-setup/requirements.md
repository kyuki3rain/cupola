# 要件定義書

## はじめに

本仕様は、Cupola プロジェクトにおける ADR（Architecture Decision Records）の整備を目的とする。設計上の意思決定を記録・共有することで、過去の議論を繰り返すことなく、将来の開発者（人間・AIエージェント双方）が意思決定の経緯を参照できる体制を構築する。

対象成果物：
- `docs/adr/` ディレクトリの作成
- ADR テンプレートファイル（`docs/adr/template.md`）の作成
- 最初の ADR（`docs/adr/001-stall-detection-event-handling.md`）の作成
- `AGENTS.md` への ADR 参照リンク追加
- `CLAUDE.md` への ADR 言及追加

## 要件

### Requirement 1: ADR ディレクトリ構造の作成

**目的:** 開発者として、設計上の意思決定を格納する標準的なディレクトリ構造が欲しい。それにより、ADR ファイルを一元管理し、検索・参照しやすい状態にする。

#### 受け入れ基準

1. The ADR setup shall `docs/adr/` ディレクトリを作成する。
2. When `docs/adr/` ディレクトリが作成されたとき、the ADR setup shall `.gitkeep` またはテンプレートファイルを配置し、空ディレクトリがリポジトリに含まれる状態を維持する。
3. The ADR setup shall ファイル名を `NNN-kebab-case-title.md` 形式（例: `001-stall-detection-event-handling.md`）に統一する。

---

### Requirement 2: ADR テンプレートの作成

**目的:** 開発者として、一貫したフォーマットの ADR テンプレートが欲しい。それにより、新規 ADR 作成時に記載すべき項目を漏らさず、統一されたフォーマットで記録できる。

#### 受け入れ基準

1. The ADR setup shall `docs/adr/template.md` を作成する。
2. The template shall 以下のセクションをすべて含む：`# ADR-NNN: タイトル`、`## ステータス`、`## コンテキスト`、`## 決定`、`## 理由`、`## 却下した代替案`。
3. The template shall `ステータス` フィールドの有効値として `Accepted`、`Superseded`、`Deprecated` を例示として記載する。
4. When 開発者が新しい ADR を作成するとき、the template shall コピー元として使用できるよう、各セクションに説明コメントを含む。

---

### Requirement 3: 最初の ADR（001）の作成

**目的:** 開発者として、`step5_stall_detection` におけるイベント処理設計の判断記録が欲しい。それにより、同じ議論が将来再燃した際に根拠を示せる。

#### 受け入れ基準

1. The ADR setup shall `docs/adr/001-stall-detection-event-handling.md` を作成する。
2. The ADR shall タイトルとして「step5_stall_detection でイベントを即時生成しない理由」を含む。
3. The ADR shall ステータスとして `Accepted` を明記する。
4. The ADR shall コンテキストセクションに、`step5_stall_detection` で stall kill が発生した後のイベント処理方式の選択問題を記述する。
5. The ADR shall 決定セクションに、「stall kill 後のイベントは次サイクルの step3 で一元処理する」という決定を記録する。
6. The ADR shall 理由セクションに、即時イベント方式（方式A）において `ProcessFailed → DesignRunning → DesignRunning` の自己遷移が発生し、stale exit guard が無効となって step3 で `ProcessFailed` が再発火し `retry_count` が二重インクリメントされるバグを説明する。
7. The ADR shall 却下した代替案セクションに、「step5 で `ProcessFailed` を即時 push する方式（方式A）」とその却下理由を記録する。

---

### Requirement 4: AGENTS.md への ADR 参照追加

**目的:** AIエージェントとして、設計判断時に参照すべき ADR の存在を知りたい。それにより、過去の意思決定を踏まえた判断が自動的に行える。

#### 受け入れ基準

1. When `AGENTS.md` が存在するとき、the ADR setup shall `docs/adr/` ディレクトリへの参照リンクまたは言及を追加する。
2. The reference shall AIエージェントが設計・実装判断を行う際に ADR を参照すべきである旨を明示する。
3. The ADR setup shall 既存の `AGENTS.md` の構造・フォーマットを破壊せず、適切なセクションに追記する。

---

### Requirement 5: CLAUDE.md への ADR 言及追加

**目的:** 開発者として、`CLAUDE.md` に ADR への言及が欲しい。それにより、Claude Code セッション開始時に ADR の存在が自動的に把握され、設計判断の一貫性が保たれる。

#### 受け入れ基準

1. When `CLAUDE.md` が存在するとき、the ADR setup shall ADR ディレクトリへの参照を追加する。
2. The mention shall `docs/adr/` のパスと、設計判断時に参照すること、を含む。
3. The ADR setup shall 既存の `CLAUDE.md` の構造・フォーマットを破壊せず、適切なセクションに追記する。
