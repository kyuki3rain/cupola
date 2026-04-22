# Implementation Plan

## Tasks Overview

テストのみの追加。プロダクションコードへの変更なし。全タスクは `devbox run test` で検証する。

---

- [ ] 1. stall_timeout 境界値単体テストの追加
- [ ] 1.1 (P) timeout=0 で全セッションが stall 検出される動作を検証する
  - セッションを複数登録し、十分な経過時間を確保した後に timeout=0 で stall 検出を呼び出す
  - 全セッションが stall として検出されることを assert する
  - `elapsed > 0` が成立する条件を確保した上で検証することをコメントで明記する
  - _Requirements: 2.4, 2.5_

- [ ] 1.2 (P) 非常に大きい timeout 値で stall が検出されないことを検証する
  - セッション登録直後に到達不可能な大きさの timeout で stall 検出を呼び出す
  - 全セッションが stall として検出されないことを assert する
  - 「長い timeout では登録直後の経過時間が到達不可能なため stall にならない」ことをコメントで明記する
  - _Requirements: 2.2, 2.5_

- [ ] 1.3 (P) 十分小さい timeout で stall 検出を安定して検証する
  - セッションの経過時間が timeout を確実に超えるよう、十分な時間差を持たせた条件を設定する
  - `Instant::now()` の精度に依存せず、elapsed が timeout を確実に超える構成でテストを実装する
  - 安定した検出のために十分な時間差を確保することをコメントで明記する
  - _Requirements: 2.3, 2.5_

- [ ] 1.4 (P) ちょうど境界（strictly greater than 判定）での動作を検証する
  - stall 検出が `>` 判定（strictly greater than）であることをコメントで確定する
  - timeout=0 での全件検出と大きい timeout での全件非検出を組み合わせて境界の意図を表現する
  - `Instant` ベースでの「ちょうど境界」を精密にシミュレートすることが困難な理由をコメントで説明する
  - _Requirements: 2.1, 2.5_

- [ ] 2. マルチ並行 issue 結合テストの追加
- [ ] 2.1 5 件の issue を並行処理し、全件の ProcessRun が正常に遷移・persist されることを検証する
  - 5 件の issue を DesignRunning 状態で同時に登録し、exited session を処理する
  - 全件の ProcessRun が Succeeded 状態に遷移し persist されることを assert する
  - _Requirements: 1.1, 1.4, 1.5_

- [ ] 2.2 複数 issue の並行処理後に DB 書き込みが正確に persist されることを検証する
  - 複数の issue を異なる状態で混在登録し exited session を処理する
  - 全 ProcessRun が Succeeded または Failed のいずれかの状態で persist され、未処理が残らないことを assert する
  - 単一プロセス内では DB が直列化されるため真の lock 競合は発生しないが、並行呼び出し後の正確性を検証する旨をコメントで明記する
  - _Requirements: 1.2, 1.4, 1.5_

- [ ] 2.3 セッション上限到達後に追加登録が reject されることを検証する
  - 上限と等しい数のセッションを登録した後に追加登録を試みる
  - 超過分の登録が reject され、セッション確保に失敗することを assert する
  - _Requirements: 1.3, 1.5_

- [ ] 3. T-RS.4 エンドツーエンドテストの追加
- [ ] 3.1 stall 検出 → プロセス kill → 次サイクルで failed になる E2E 経路を検証する
  - stall timeout を超えた状態のセッションに kill を実行し、プロセスが終了することを確認する
  - 次の polling サイクルで ProcessRun が Failed 状態に遷移し persist されることを assert する
  - test-spec.md の T-RS.4 仕様への対応であることをコメントで明記する
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 4. 品質チェックと修正
- [ ] 4.1 リントエラーがなく、テストコードの品質が保たれていることを確認する
  - 新規テスト関数内の expect 呼び出しに意図を示すメッセージが付与されていることを確認する
  - _Requirements: 1.4, 2.5, 3.3_

- [ ] 4.2 全テストが green であることを確認する
  - 新規テスト 8 本（単体 4 本 + 統合 4 本）が全て pass することを確認する
  - 既存テストへのリグレッションがないことを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2_
