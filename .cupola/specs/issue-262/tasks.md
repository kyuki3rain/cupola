# Implementation Plan

- [ ] 1. panic 発生時の PID ファイル後始末を保証する
- [ ] 1.1 panic 発生時にも PID ファイルが確実に削除されるようにする
  - 予期しない異常終了時でも、実行中プロセスを示す PID ファイルが残り続けないようにする
  - panic の内容が運用時に把握できるよう、既存のエラー出力や診断情報を維持する
  - PID ファイルの削除に失敗しても、追加の異常終了を起こさない安全な後始末にする
  - 既存の panic 時の挙動を不必要に変えず、必要な cleanup だけを追加する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 1.2 通常起動時にも panic 時 cleanup が有効になるようにする
  - PID ファイルを作成して稼働する起動経路では、panic 時の後始末が同様に働くようにする
  - _Requirements: 1.5_

- [ ] 1.3 子プロセス起動時にも panic 時 cleanup が有効になるようにする
  - バックグラウンド実行や子プロセス側の起動経路でも、panic 時に PID ファイルが残留しないようにする
  - _Requirements: 1.5_

- [ ] 2. テストの追加
- [ ] 2.1 (P) panic 時 cleanup のユニットテストを追加する
  - `install_panic_hook` 呼び出し後、実際に `panic!()` を発生させて hook を起動し、`std::panic::catch_unwind` でテストプロセスを継続させつつ PID ファイルが削除されることを確認する
  - PID ファイルが存在しない場合も、同様に `catch_unwind` で panic を捕捉した上で hook 経由のクリーンアップ処理が安全に完了することを確認する
  - `std::panic::take_hook()` で hook を直接取り出して偽の `PanicHookInfo` / `PanicInfo` を渡す方法は使わない
  - _Requirements: 2.3_

- [ ] 2.2 (P) panic 時 PID ファイルクリーンアップのインテグレーションテストを追加する
  - `tests/integration_test.rs` または `tests/scenarios.rs` にテストを追加する
  - スレッドを spawn して PID ファイルを書き込み、`install_panic_hook` を設定後にそのスレッド内で実際に `panic!()` させ、`join` で終了を待つ
  - 実際の起動に近い条件で panic を発生させ、終了後に PID ファイルが削除されていることを検証する
  - 起動モードの違いにかかわらず、期待した cleanup が行われることを確認する
  - _Requirements: 2.1, 2.2_
