# Requirements Document

## Introduction
cupola の INFO レベルログを改善し、運用時のデバッグ・監視に必要な重要イベント（状態遷移、PR 作成、fixing 完了、プロセス終了）を記録する。現状では INFO ログがプロセス起動時のみに限られており、ステートマシンの状態遷移や PR 作成といった重要な操作がログに残らないため、E2E テストや運用時の問題追跡が困難になっている。

## Requirements

### Requirement 1: 状態遷移ログ
**Objective:** 運用者として、Issue のステートマシン状態遷移を INFO ログで確認したい。これにより、各 Issue がどの工程にいるかをログだけで追跡できるようになる。

#### Acceptance Criteria
1. When TransitionUseCase が状態遷移を適用した時, Cupola shall 遷移元状態・遷移先状態・issue_number を含む INFO ログを出力する
2. When 状態遷移が発生した時, Cupola shall ログメッセージに `from` と `to` のフィールドを構造化ログとして含める
3. When IssueClosed イベントにより Cancelled 状態へ遷移した時, Cupola shall 通常の状態遷移と同様に INFO ログを出力する

### Requirement 2: PR 作成ログ
**Objective:** 運用者として、PR 作成の成功を INFO ログで確認したい。これにより、どの Issue に対してどの PR が作成されたかをログから特定できるようになる。

#### Acceptance Criteria
1. When GitHub 上に PR が正常に作成された時, Cupola shall pr_number・head ブランチ・base ブランチを含む INFO ログを出力する
2. When 既存の PR が検出されスキップされた時, Cupola shall スキップした旨と既存の pr_number を含む INFO ログを出力する

### Requirement 3: fixing 後処理完了ログ
**Objective:** 運用者として、レビュー指摘への自動修正・返信・resolve の完了を INFO ログで確認したい。これにより、fixing サイクルの進行状況を把握できるようになる。

#### Acceptance Criteria
1. When fixing 後処理（スレッド返信 + resolve）が全て完了した時, Cupola shall 処理したスレッド数と issue_number を含む INFO ログを出力する
2. When 個別のスレッド返信が成功した時, Cupola shall thread_id と返信成功の旨を含む INFO ログを出力する
3. When 個別のスレッド resolve が成功した時, Cupola shall thread_id と resolve 成功の旨を含む INFO ログを出力する

### Requirement 4: Claude Code プロセス終了ログ
**Objective:** 運用者として、Claude Code プロセスの正常終了を INFO ログで確認したい。これにより、プロセスの実行結果をログから迅速に確認できるようになる。

#### Acceptance Criteria
1. When Claude Code プロセスが正常終了した時（exit_code = 0）, Cupola shall exit_code と issue_number を含む INFO ログを出力する
2. When Claude Code プロセスが異常終了した時（exit_code ≠ 0）, Cupola shall exit_code・issue_number・失敗の旨を含む INFO ログを出力する
