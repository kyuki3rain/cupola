# 実装計画

- [ ] 1. リトライ上限到達時に必要な失敗情報を保持できるようにする
- [ ] 1.1 上限到達コメント用の Effect がプロセスタイプと連続失敗回数を持てるようにする
  - `Effect::PostRetryExhaustedComment` を構造体バリアントに変更し、`process_type: ProcessRunType` と `consecutive_failures: u32` フィールドを追加する
  - `priority()` と `is_best_effort()` のパターンマッチをフィールド無視（`..`）付きに更新する
  - `effect.rs` 内の既存テスト（構築・priority・is_best_effort）を新バリアント形式に合わせて修正する
  - _Requirements: 2.1_

- [ ] 2. 各プロセスの上限到達時に失敗情報を正しく引き継げるようにする
- [ ] 2.1 上限到達を表す遷移でプロセスタイプと連続失敗回数を受け取れるようにする
  - 関数シグネチャを `(effects, process_type: ProcessRunType, consecutive_failures: u32)` に変更する
  - 関数内で `Effect::PostRetryExhaustedComment { process_type, consecutive_failures }` を push するよう修正する
  - _Requirements: 2.2_

- [ ] 2.2 (P) Init プロセスで上限到達時の失敗回数を引き継げるようにする
  - `snap.processes.init` から `consecutive_failures` を取り出して上限到達遷移に渡す
  - `ProcessRunType::Init` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.3 (P) Design プロセスで上限到達時の失敗回数を引き継げるようにする
  - `snap.processes.design` から `consecutive_failures` を取り出して渡す
  - `ProcessRunType::Design` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.4 (P) Impl プロセスで上限到達時の失敗回数を引き継げるようにする
  - `snap.processes.impl_` から `consecutive_failures` を取り出して渡す
  - `ProcessRunType::Impl` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.5 (P) そのほかのプロセスタイプでも上限到達時の失敗回数を一貫して引き継げるようにする
  - 上限到達遷移を呼ぶ残り全箇所（line 371, 738 周辺）を、対応する `ProcessSnapshot` の値で更新する
  - _Requirements: 1.3, 2.2_

- [ ] 3. 上限到達コメント生成で保持済みの失敗回数を利用できるようにする
- [ ] 3.1 コメント生成時に Effect のペイロードから失敗回数を読み取るようにする
  - パターンを `Effect::PostRetryExhaustedComment { consecutive_failures, .. }` に変更し `count = consecutive_failures` を使用する
  - `count_total_failures` ヘルパー関数を削除する
  - _Requirements: 1.1, 1.2, 2.3_

- [ ] 4. テストを追加・修正して正確性を検証する
- [ ] 4.1 (P) 上限到達コメント用 Effect の新しい形を既存テストで検証できるようにする
  - `PostRetryExhaustedComment` の構築テストに `process_type` と `consecutive_failures` を含める
  - `priority()` / `is_best_effort()` のテストを新形式に合わせる
  - _Requirements: 2.1, 4.1_

- [ ] 4.2 (P) 各プロセスタイプで上限到達時の失敗回数引き継ぎを検証できるようにする
  - Init / Design / Impl それぞれで `consecutive_failures >= max_retries` のとき、エフェクトに正しい `process_type` と `consecutive_failures` が含まれることを assert する
  - _Requirements: 1.3, 4.2_

- [ ] 4.3 上限到達コメントで新しいペイロードが使われることを検証する
  - `Effect::PostRetryExhaustedComment { process_type: ProcessRunType::Design, consecutive_failures: 3 }` を渡したとき GitHub コメントの `count` が `3` であることを検証するユニットテストを追加する
  - _Requirements: 1.1, 4.1_

- [ ] 5. 変更後も品質ゲートを満たすことを確認する
- [ ] 5.1 静的解析とテストが警告・失敗なく完了することを確認する
  - `devbox run clippy` と `devbox run test` を実行してすべて通過することを確認する
  - `count_total_failures` 削除後の dead code 警告がないことを確認する
  - `cargo clippy -- -D warnings` がエラーゼロで終了することを確認する
  - _Requirements: 4.3_
