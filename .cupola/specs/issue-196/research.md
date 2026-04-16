# リサーチ＆設計判断ログ

---
**目的**: ディスカバリーで得た知見、アーキテクチャ調査、技術設計の根拠を記録する。

---

## サマリー

- **フィーチャー**: `issue-196` — `PostRetryExhaustedComment` 連続失敗数不一致の修正
- **ディスカバリースコープ**: Extension（既存システムへの変更）
- **主要知見**:
  - `Effect::PostRetryExhaustedComment` は現在ペイロードを持たないユニット型バリアントである
  - `decide.rs` は `consecutive_failures` を `ProcessSnapshot` から参照してリトライ上限を判定しており、その値はエフェクト生成時点で利用可能
  - `execute.rs` の `count_total_failures` は全タイプ横断の累計であり、ドキュメントの「連続失敗数」と意味が異なる
  - `ProcessRunRepository::count_consecutive_failures` トレイトメソッドは既存で、`type_` と `since` を引数に取る
  - エフェクトにペイロードを追加する方法（Option A）が最も侵襲が小さく、追加 DB クエリも不要

---

## リサーチログ

### トピック: Effect バリアントの構造と変更コスト

- **きっかけ**: `PostRetryExhaustedComment` にプロセスタイプと連続失敗数を持たせるために、既存の Effect 設計を把握する必要があった
- **調査ソース**: `src/domain/effect.rs`、`src/application/polling/execute.rs`、`src/domain/decide.rs`
- **知見**:
  - `Effect` は `Clone + PartialEq + Eq` を derives。フィールド追加は既存テストのパターンマッチ部分を更新する必要がある
  - `PostRetryExhaustedComment` はユニット型（フィールドなし）。`SpawnProcess` はすでに `{ type_, causes, pending_run_id }` を持つ先例がある
  - `go_cancelled_retry_exhausted` は各 `decide_*_running` から呼ばれており、呼び出し時点で `ProcessSnapshot.consecutive_failures` が参照可能
- **影響**: Effect バリアントを構造体バリアントに変更すると、`effect.rs` のテスト（構築・パターンマッチ）と `execute.rs` のマッチアーム、`decide.rs` の呼び出し側すべてを更新する必要がある

### トピック: どのタイミングで連続失敗数を取得するか

- **きっかけ**: `execute.rs` でどのように回数を取得するかの設計選択肢を評価
- **調査ソース**: `src/application/port/process_run_repository.rs`、`src/adapter/outbound/sqlite_process_run_repository.rs`
- **知見**:
  - Option A（エフェクトにペイロード埋め込み）: `decide.rs` がすでに `consecutive_failures` を保持。追加 DB クエリ不要。ドメイン層の純粋性を保ちつつ Execute phase への情報伝達が明確になる
  - Option B（execute.rs から DB クエリ）: `PostRetryExhaustedComment` に `process_type` だけ持たせ、`count_consecutive_failures` を実行フェーズでクエリ。追加の async IO が発生するが、ドメイン層の変更を最小化できる
  - Option C（ドキュメント修正のみ）: コードの意味的誤りを温存するため、否
- **影響**: Option A を選択。`decide.rs` はすでに `consecutive_failures` を知っているため、DB クエリの追加は不要で副作用が少ない

### トピック: decide.rs における go_cancelled_retry_exhausted の呼び出し箇所

- **きっかけ**: どこを変更するかの影響範囲を特定するため
- **調査ソース**: `grep go_cancelled_retry_exhausted src/domain/decide.rs`
- **知見**:
  - Init: `decide_init_running`（line 147） → `snap.processes.init`
  - Design: `decide_design_running`（line 204） → `snap.processes.design`
  - Impl: `decide_impl_running`（line 558） → `snap.processes.impl_`
  - DesignFix / ImplFix 系: 同様に `snap.processes.design_fix` / `snap.processes.impl_fix` の可能性あり（line 371, 738）
  - 各呼び出しはそれぞれの `ProcessSnapshot` を参照しているため、`consecutive_failures` は常に利用可能
- **影響**: `go_cancelled_retry_exhausted` のシグネチャを変更し、`process_type: ProcessRunType` と `consecutive_failures: u32` を受け取るよう更新する必要がある

---

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク / 制限 | 備考 |
|------------|------|------|---------------|------|
| A: Effect ペイロード追加 | Effect バリアントに `process_type` + `consecutive_failures` を埋め込む | 追加 DB クエリ不要、ドメイン自完結 | Effect の全マッチ箇所の更新が必要 | **採用** |
| B: execute.rs でクエリ | Effect に `process_type` のみ追加し、execute 側で `count_consecutive_failures` を呼ぶ | Effect 変更が最小 | 追加の async IO。execute 層でドメイン知識を持つ | 否 |
| C: ドキュメント修正のみ | docs を「累計失敗回数」に変更 | 実装変更なし | 意味的に誤った挙動を温存。ユーザー体験が悪い | 否 |

---

## 設計判断

### 判断: Effect::PostRetryExhaustedComment にペイロードを追加する

- **背景**: 現在 `PostRetryExhaustedComment` はペイロードなし。decide.rs が持つ情報を execute.rs に伝える手段がなく、execute.rs が独自に DB クエリで別の（意味的に誤った）値を取得している
- **検討した代替案**:
  1. Option A: Effect にペイロード埋め込み
  2. Option B: execute.rs 側でクエリ追加
- **採用アプローチ**: Option A。`Effect::PostRetryExhaustedComment { process_type: ProcessRunType, consecutive_failures: u32 }` に変更
- **根拠**: `decide.rs` はすでに両フィールドを保持しており、`SpawnProcess` が先例として構造体バリアントを使っている。追加クエリ不要で execute.rs がシンプルになる
- **トレードオフ**: Effect enum のパターンマッチを使っている箇所（テスト含む）をすべて更新する必要がある
- **フォローアップ**: `count_total_failures` ヘルパーは使われなくなるため削除し、dead code 警告を除去する

---

## リスクと緩和策

- `Effect` 変更に伴う広範なパターンマッチ更新 → CI（cargo clippy / cargo test）ですべての見落としを検出できる
- `go_cancelled_retry_exhausted` のシグネチャ変更による呼び出し漏れ → Rust コンパイラが型不一致を検出する
- DesignFix / ImplFix 系の呼び出し箇所を見落とすリスク → `grep go_cancelled_retry_exhausted` で全箇所を特定済み

---

## 参照

- `src/domain/effect.rs` — Effect 列挙型の定義
- `src/domain/decide.rs` — リトライ上限判定と go_cancelled_retry_exhausted の呼び出し
- `src/application/polling/execute.rs` — エフェクト実行、count_total_failures の実装
- `src/application/port/process_run_repository.rs` — count_consecutive_failures トレイト定義
- `docs/architecture/effects.md` — PostRetryExhaustedComment の仕様記述
- `locales/ja.yml` — コメントテンプレート（%{count} %{error}）
