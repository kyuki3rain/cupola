# Requirements Document

## Introduction

`polling_use_case.rs` のメインポーリングループ内の `tokio::select!` はデフォルトで擬似ランダム選択を行う。`tick.tick()` と OS シグナル受信が同一サイクルで ready になった場合、tick アームが選択されて不要な 1 サイクルが実行される競合ウィンドウが存在する。`biased;` キーワードを追加してシグナルアームを先頭に配置することで、シグナルが常に優先されるようにし、シャットダウン直前の GitHub 副作用（コメント投稿・PR 作成・issue クローズ等）の余分な発火を防ぐ。

## Requirements

### Requirement 1: tokio::select! の biased 化によるシグナル優先制御

**Objective:** オペレーターとして、デーモン停止シグナル送信時に余分なポーリングサイクルが実行されない仕組みがほしい。そうすることで、シャットダウン直前の意図しない GitHub 副作用を防止できる。

#### Acceptance Criteria

1.1. The polling use case shall `tokio::select!` に `biased;` キーワードを追加し、アームの評価を宣言順の固定優先度で行う。

1.2. When `tick.tick()` と OS シグナル（SIGINT・SIGTERM・SIGHUP）が同時に ready になったとき、the polling use case shall シグナルアームを tick アームより優先して選択する。

1.3. The polling use case shall SIGINT・SIGTERM・SIGHUP の各シグナルアームを、tick アームより前に宣言された順序で配置する。

1.4. If `biased;` を適用した後も既存のシグナルハンドラロジック（SIGINT 2 回目の強制シャットダウン・SIGTERM/SIGHUP のグレースフルシャットダウン）が変更なく動作すること。

### Requirement 2: テスト（任意）

**Objective:** 開発者として、シグナル優先制御の動作を確認できるテストがほしい。そうすることで、将来の変更でリグレッションを検出できる。

#### Acceptance Criteria

2.1. Where テストの追加が可能な場合、the polling use case shall シャットダウンシナリオ（シグナル受信時に余分なサイクルが走らないこと）を検証する結合テストを持つ。

2.2. The polling use case shall `biased;` 適用後も既存のテストがすべてパスする。
