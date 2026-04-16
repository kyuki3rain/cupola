# 実装計画

- [ ] 1. Effect バリアントにペイロードを追加する
- [ ] 1.1 `Effect::PostRetryExhaustedComment` を構造体バリアントに変更する
  - `process_type: ProcessRunType` と `consecutive_failures: u32` フィールドを追加する
  - `priority()` と `is_best_effort()` のパターンマッチをフィールド無視（`..`）付きに更新する
  - `effect.rs` 内の既存テスト（構築・priority・is_best_effort）を新バリアント形式に合わせて修正する
  - _Requirements: 2.1_

- [ ] 2. `decide.rs` の呼び出し箇所をすべて更新する
- [ ] 2.1 `go_cancelled_retry_exhausted` のシグネチャに `process_type` と `consecutive_failures` を追加する
  - 関数シグネチャを `(effects, process_type: ProcessRunType, consecutive_failures: u32)` に変更する
  - 関数内で `Effect::PostRetryExhaustedComment { process_type, consecutive_failures }` を push するよう修正する
  - _Requirements: 2.2_

- [ ] 2.2 (P) `decide_init_running` の上限到達箇所を更新する
  - `snap.processes.init` から `consecutive_failures` を取り出して `go_cancelled_retry_exhausted` に渡す
  - `ProcessRunType::Init` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.3 (P) `decide_design_running` の上限到達箇所を更新する
  - `snap.processes.design` から `consecutive_failures` を取り出して渡す
  - `ProcessRunType::Design` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.4 (P) `decide_impl_running` の上限到達箇所を更新する
  - `snap.processes.impl_` から `consecutive_failures` を取り出して渡す
  - `ProcessRunType::Impl` を渡す
  - _Requirements: 1.3, 2.2_

- [ ] 2.5 (P) その他のプロセスタイプ（DesignFix / ImplFix 等）の上限到達箇所を更新する
  - `go_cancelled_retry_exhausted` を呼ぶ残り全箇所（line 371, 738 周辺）を対応する ProcessSnapshot の値で更新する
  - _Requirements: 1.3, 2.2_

- [ ] 3. `execute.rs` をペイロード利用に切り替える
- [ ] 3.1 `PostRetryExhaustedComment` マッチアームをペイロード読み取りに変更する
  - パターンを `Effect::PostRetryExhaustedComment { consecutive_failures, .. }` に変更し `count = consecutive_failures` を使用する
  - `count_total_failures` ヘルパー関数を削除する
  - _Requirements: 1.1, 1.2, 2.3_

- [ ] 4. テストを追加・修正して正確性を検証する
- [ ] 4.1 (P) `effect.rs` のテストを新バリアントに対応させる
  - `PostRetryExhaustedComment` の構築テストに `process_type` と `consecutive_failures` を含める
  - `priority()` / `is_best_effort()` のテストを新形式に合わせる
  - _Requirements: 2.1, 4.1_

- [ ] 4.2 (P) `decide.rs` に各プロセスタイプの上限到達テストを追加・修正する
  - Init / Design / Impl それぞれで `consecutive_failures >= max_retries` のとき、エフェクトに正しい `process_type` と `consecutive_failures` が含まれることを assert する
  - _Requirements: 1.3, 4.2_

- [ ] 4.3 `execute.rs` に PostRetryExhaustedComment の新ペイロードを使ったテストを追加する
  - `Effect::PostRetryExhaustedComment { process_type: ProcessRunType::Design, consecutive_failures: 3 }` を渡したとき GitHub コメントの `count` が `3` であることを検証するユニットテストを追加する
  - _Requirements: 1.1, 4.1_

- [ ] 5. CI チェックを通過させる
- [ ] 5.1 `devbox run clippy` と `devbox run test` を実行してすべて通過することを確認する
  - `count_total_failures` 削除後の dead code 警告がないことを確認する
  - `cargo clippy -- -D warnings` がエラーゼロで終了することを確認する
  - _Requirements: 4.3_
