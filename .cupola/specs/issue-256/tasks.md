# Implementation Plan

- [x] 1. Idle 状態のときのみラベルアクター検証を行うよう変更する
- [x] 1.1 ポーリング対象 issue の状態を受け取り、Idle 状態のときのみ外部 API を呼び出してラベルアクターを検証するよう条件分岐を追加する
  - Idle 以外の状態かつ ready_label が付いている場合は、外部 API 呼び出しをスキップし即座に「信頼されていない」として返す
  - Idle 状態かつ ready_label が付いている場合のみ、既存のラベルアクター信頼検証フローを実行する
  - 呼び出し元は issue の現在の状態をラベルアクター検証処理に渡す
  - _Requirements: 1.1, 1.2, 1.4_

- [x] 2. テストを更新・追加する
- [x] 2.1 (P) Idle 状態を前提とする既存のラベルアクター検証テストを更新する
  - closed issue のテストを、状態情報を受け渡す新しい形式に合わせて更新する
  - Idle + ready_label あり のテストで Idle 状態を明示的に指定するよう更新する
  - Idle + ready_label なし のテストで Idle 状態を明示的に指定するよう更新する
  - _Requirements: 2.1, 2.2, 2.3, 3.2, 3.3_

- [x] 2.2 (P) 非 Idle 全状態でラベルアクター外部 API が呼ばれないことを検証するテストを追加する
  - ready_label が付いていても、非 Idle 状態では外部 API 呼び出しが一切発生しないことを検証する
  - `InitializeRunning`、`DesignRunning`、`DesignReviewWaiting`、`DesignFixing`、`ImplementationRunning`、`ImplementationReviewWaiting`、`ImplementationFixing`、`Completed`、`Cancelled` の全状態を網羅する
  - _Requirements: 1.2, 3.1_
