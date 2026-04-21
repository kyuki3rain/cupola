# 調査・設計判断メモ

---
**機能**: issue-311 — プロセス stdout/stderr ストリーム書き込み化
**調査スコープ**: Extension（既存システムへの拡張）
**主要発見事項**:
- `register` 直後に `update_run_id` が常に呼ばれるため、一時ファイルの利用期間は極めて短い
- `std::io::copy` + `LineWriter` の組み合わせで tail -f 互換の粒度を確保できる
- テスト側は `ExitedSession` の構造変更に伴い、ファイルフィクスチャを使う形に全面移行が必要

---

## 調査ログ

### run_id の確定タイミング

- **背景**: Issue が「`register` 時点で run_id が確定しているかを要確認」と指摘
- **調査先**: `src/application/polling/execute.rs` の `spawn_process` 関数
- **発見事項**:
  - `spawn_process` は `prepare_process_spawn(...)` を呼び出し、`(child, run_id, pid)` を受け取る
  - 直後に `session_mgr.register(issue.id, issue.state, child)` を呼び、すぐ次行で `session_mgr.update_run_id(issue.id, run_id)` を呼ぶ
  - つまり、`register` と `update_run_id` は同じ同期ブロック内で連続呼び出しされる
  - run_id が不確定な状態でセッションが長時間稼働することはない
- **影響**: 一時ファイルのリネームは必須だが、実際には極めて短期間（サブミリ秒）しか一時パスを使わない

### 既存のログ書き込み方式

- **背景**: `dump_session_io` の役割とタイミングを把握
- **調査先**: `src/application/polling/resolve.rs:122-137`
- **発見事項**:
  - `dump_session_io` はプロセス完了後に in-memory バッファ全体を一括ファイル書き出しする
  - `session.stdout` は `collect_exited` が `stdout_handle.join()` して得た `String`
  - 30 分稼働の stderr は最大数十 MB になりえる
- **影響**: ストリーム化後は `dump_session_io` が不要になるため削除できる

### LineWriter vs BufWriter + 定期 flush

- **背景**: `tail -f` 互換性のための書き込み粒度
- **発見事項**:
  - `std::io::LineWriter` は改行ごとに自動フラッシュするため、tail -f での観察が自然になる
  - Claude Code の stderr は通常ラインバッファリングされているため、LineWriter との相性が良い
  - stdout は `--output-format json` により最終 1 行のみ出力されるため、LineWriter でも BufWriter でも実用上差異はない
  - `std::io::copy` は内部で 8 KB チャンクを使うが、LineWriter により改行でフラッシュが走る
- **決定**: `LineWriter<BufWriter<File>>` を採用（BufWriter でバッファリングしつつ改行ごとフラッシュ）

### tempfile クレートの利用可否

- **背景**: `run_id` 未確定時の一時ファイル戦略
- **調査先**: `Cargo.toml` の依存関係
- **発見事項**: `tempfile` クレートはすでに `dev-dependencies` に存在する（`src/application/io.rs` のテストで使用）
  - ただし `dependencies` には含まれていない
  - 本番コードで `NamedTempFile` を使うには `dependencies` への追加が必要
- **代替案検討**:
  - A: `tempfile` を `dependencies` に追加して `NamedTempFile` を使う
  - B: 一時ファイルを `issue_id` + タイムスタンプで命名し手動管理する
  - C: `register` 時に run_id を渡すよう `spawn_process` 側の呼び出しを変更する
- **決定**: **C を採用**。`spawn_process` は run_id を取得してから `register` を呼ぶため、`register(issue_id, state, child, run_id)` にシグネチャを変更するのが最もシンプル。`update_run_id` を完全に廃止できる（ただし後述の理由で API は維持）
  - 実際には `register` に `run_id: i64` を追加し、ファイルパスを即座に確定させる
  - `update_run_id` は呼び出し箇所が削除されるが、`update_log_id` と対称性を保つために存在は維持してもよい。ただしファイルリネーム処理は不要

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク／制限 | 備考 |
|---|---|---|---|---|
| A: in-memory → 完了後ダンプ（現状） | JoinHandle<String> でメモリ蓄積、完了後一括書き出し | 実装シンプル | OOM リスク、デバッグ不可 | 不採用（現状） |
| B: bounded ring buffer (stderr のみ) | stderr の末尾 N MB だけ保持 | 最小工数 | stdout は依然 in-memory、設計整合なし | 不採用 |
| C: ストリーム to file（採用） | register 時にリーダースレッド起動、即座にファイルへ copy | OOM 解消、tail -f 対応、設計整合 | テスト移行コスト | 採用 |

## 設計決定

### 決定: `register` シグネチャへの run_id 追加

- **背景**: run_id が必要なのにregister 呼び出し時点では不明、という問題の解決策
- **検討案**:
  1. tempfile::NamedTempFile を依存に追加し update_run_id でリネーム
  2. issue_id + タイムスタンプで一時ファイル名を構築
  3. `spawn_process` 側で run_id を先に確定させ register に渡す
- **採用方針**: 案3。`spawn_process` の実装では run_id は `prepare_process_spawn` の戻り値として既に確定している。`register(issue_id, state, child, run_id)` に変更すれば、ファイルパスを即座に確定できる
- **トレードオフ**: `register` の呼び出しシグネチャが変わるため、テストコードの `register` 呼び出しもすべて更新が必要。ただしシンプルさを得られる
- **フォローアップ**: `update_run_id` は `update_log_id` と対称性があるが、ファイルリネームが不要になれば実質的に空になる。呼び出し箇所を `spawn_process` から削除する

### 決定: 書き込みフォールバック戦略

- **背景**: ファイル作成失敗時の安全な動作
- **採用方針**: リーダースレッド内での `File::create` 失敗は `tracing::error!` のみで継続（プロセスは止めない）。resolve フェーズでのファイル読み取り失敗は `mark_failed` で強制終了
- **理由**: ログなしで PR を作るより安全。空 PR body で誤動作するリスクを排除

## リスクと緩和策

- **リネーム競合**: リーダースレッドが書き込み中にリネームするリスク → `rename` は OS レベルのアトミック操作のためファイルディスクリプタは維持される（Unix）
- **テスト移行コスト**: `ExitedSession` の `stdout`/`stderr` フィールドを参照するテストが多数ある → ファイルフィクスチャを用いたヘルパー関数を用意して移行する
- **Windows 非対応**: `rename` 中に他プロセスがファイルを開いている場合、Windows では失敗する可能性がある → 本プロジェクトは macOS/Linux ターゲットのため問題なし

## 参照

- `src/application/session_manager.rs` — 現在の JoinHandle<String> 実装
- `src/application/polling/resolve.rs:122-137` — `dump_session_io` 実装
- `src/application/polling/execute.rs:584-604` — `spawn_process` の register 呼び出し順序
- `docs/architecture/polling-loop.md:52-53` — 「stdout/stderr 全文を log に書き出す」設計記述
