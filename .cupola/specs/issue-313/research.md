# Research & Design Decisions

## Summary
- **Feature**: `issue-313` — polling loop の tokio::select! を biased 化してシグナル優先制御を実現する
- **Discovery Scope**: Simple Addition（単一ファイルの局所変更）
- **Key Findings**:
  - `tokio::select!` の `biased;` キーワードはアームの評価順を擬似ランダムから宣言順の固定優先度へ変更する
  - 変更対象は `src/application/polling_use_case.rs` の `run()` メソッド内 1 箇所のみ
  - 新規依存クレートは不要（tokio は既存依存）
  - 孤児プロセスの懸念は既存の `graceful_shutdown` + `kill_all()` で解消済み

## Research Log

### tokio::select! の biased 動作

- **Context**: デフォルトの `tokio::select!` は複数アームが同時 ready のとき擬似ランダムに選択する。これによりシグナルと tick が競合した場合に tick が選ばれる可能性がある。
- **Sources Consulted**: tokio ドキュメント（`biased;` キーワードの仕様）
- **Findings**:
  - `biased;` を先頭に記述すると、アームは宣言された順に評価される
  - 最初に ready と判定されたアームが必ず選択される（フォールスルーなし）
  - 実行時オーバーヘッドはほぼゼロ
- **Implications**: シグナルアームを先頭に配置するだけでシグナル優先が保証される

### 変更スコープの確認

- **Context**: 変更が他コンポーネントに波及するか確認
- **Sources Consulted**: `src/application/polling_use_case.rs`（コードリーディング）
- **Findings**:
  - `tokio::select!` は `run()` メソッド内の 1 箇所のみ（行 213–244）
  - `graceful_shutdown` ロジックはシグナルアームのハンドラ内部にあり、アーム順変更で影響を受けない
  - SIGINT の 2 回目カウント (`sigint_count`) はアーム内ローカル変数で管理されており、順序変更で影響なし
- **Implications**: 変更範囲は `tokio::select!` ブロックの宣言順の並べ替えと `biased;` 追加のみ

### race condition の実害評価

- **Context**: 修正しない場合の実害を評価
- **Findings**:
  - シャットダウン直前の余分な 1 サイクルで GitHub コメント投稿・PR 作成・close_issue 等の副作用が発生し得る
  - 再起動後に同一 Effect が再発火される可能性がある
  - 孤児プロセスは `graceful_shutdown` の `kill_all()` + 10 秒 wait で回収済みのため問題なし

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク / 制限 |
|-----------|------|------|--------------|
| `biased;` キーワード追加 | select! に biased を付加してシグナルアームを先頭に配置 | 最小コスト・tokio 標準機能 | なし |
| shutdown フラグを `run_cycle` 冒頭でチェック | bool フラグでサイクル開始時にシャットダウン判定 | 明示的な制御 | `biased;` で十分なため重複防御で複雑化する |

## Design Decisions

### Decision: `biased;` を使用してシグナル優先順位を固定する

- **Context**: tick と signal が同時 ready になったとき、signal を常に優先したい
- **Alternatives Considered**:
  1. `biased;` + アーム順変更 — tokio 標準機能で最小変更
  2. shutdown フラグによる `run_cycle` 内チェック — 追加変数・追加チェックが必要で複雑化
- **Selected Approach**: `biased;` を `tokio::select!` に追加し、シグナルアーム（SIGINT → SIGTERM → SIGHUP）を tick アームより先に宣言する
- **Rationale**: 修正コストが極小で、tokio の公式機能を使うため信頼性が高い
- **Trade-offs**: `biased;` によりシグナルが常に tick より先に評価されるが、tick が届かなくなるシナリオはシグナル受信時のシャットダウン処理のみで正常動作
- **Follow-up**: 既存テストがすべてパスすることを確認する

## Risks & Mitigations

- tick の starving リスク — `biased;` 適用後、シグナルが連続して届く場合 tick が実行されなくなる可能性があるが、シグナル受信後はすぐに `break` または `return` するため実害なし
- テスト困難性 — race window の再現が困難なため、結合テストは任意扱いとする

## References

- tokio select! biased ドキュメント: https://docs.rs/tokio/latest/tokio/macro.select.html#fairness
