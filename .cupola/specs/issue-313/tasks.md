# Implementation Plan

- [ ] 1. tokio::select! を biased 化してシグナルアームを先頭に配置する
  - `PollingUseCase::run()` 内の `tokio::select!` に `biased;` キーワードを追加する
  - SIGINT・SIGTERM・SIGHUP の各シグナルアームを tick アームより前に並べ替える
  - 各シグナルアーム内のハンドラロジック（`graceful_shutdown` 呼び出し・`sigint_count` 更新）は変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. 既存テストで動作を検証する
  - `devbox run test` でテストスイート全体がパスすることを確認する
  - `devbox run clippy` で Clippy 警告がないことを確認する
  - _Requirements: 2.2_

- [ ]* 3. シャットダウンシナリオの結合テストを追加する（任意）
  - シグナル受信時に余分なポーリングサイクルが実行されないことを検証するテストを `tests/` に追加する
  - race window の再現は困難なため、実現可能な範囲でテストを設計する
  - _Requirements: 2.1_
