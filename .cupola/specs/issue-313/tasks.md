# Implementation Plan

- [x] 1. ポーリングループで同時 ready 時にシグナルを優先するようにする
  - シグナルと tick が同時 ready になった場合に、常にシグナルが選択されるよう優先制御を追加する
  - SIGINT・SIGTERM・SIGHUP の各シグナルを tick より高い優先度で評価する
  - 既存のシグナルハンドラおよびシャットダウンロジックは変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. 既存テストで動作を検証する
  - `devbox run test` でテストスイート全体がパスすることを確認する
  - `devbox run clippy` で Clippy 警告がないことを確認する
  - _Requirements: 2.2_

- [ ]* 3. シャットダウンシナリオの結合テストを追加する（任意）
  - シグナル受信時に余分なポーリングサイクルが実行されないことを検証するテストを `tests/` に追加する
  - race window の再現は困難なため、実現可能な範囲でテストを設計する
  - _Requirements: 2.1_
