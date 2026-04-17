# Implementation Plan

- [ ] 1. SIGHUP シグナルハンドラーの追加
- [ ] 1.1 (P) polling_use_case.rs に SIGHUP ハンドラーを追加する
  - `run()` 関数内で `let mut sighup = signal::unix::signal(SignalKind::hangup())?;` を `sigterm` と同じ位置に追加する
  - `tokio::select!` ブロックに `_ = sighup.recv() => { tracing::info!("received SIGHUP, shutting down (config reload is not supported, please restart to apply config changes)..."); break; }` を追加する
  - 既存の SIGTERM / SIGINT アームと同じ構造を踏襲する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 1.2 (P) SIGHUP ハンドラーのユニットテストを追加する
  - `polling_use_case.rs` の `#[cfg(test)]` ブロックに SIGHUP 受信によるシャットダウン開始の検証テストを追加する
  - SIGTERM テストがある場合はそのパターンを参照して一貫性を保つ
  - _Requirements: 1.1, 1.3_

- [ ] 2. (P) ドキュメント更新
  - `docs/commands/start.md` に「シグナルハンドリング」セクションを追加する
  - SIGTERM / SIGINT / SIGHUP の各シグナルに対する挙動（グレースフルシャットダウン）を記述する
  - SIGHUP による config reload は現時点では実装されておらず、config（`cupola.toml`）を変更した場合は daemon の再起動が必要であることを明記する
  - 将来的な config reload 実装は別 Issue で対応する旨を記述する
  - _Requirements: 2.1, 2.2, 2.3_
