# Requirements Document

## Introduction

cupola daemon は現在 SIGTERM / SIGINT をハンドリングしているが、SIGHUP はハンドリングしていない。Unix デーモンの慣例では SIGHUP は config reload に使われるが、cupola では SIGHUP を受信するとデフォルト挙動（即時プロセス終了）になる。

本仕様は **Option B（ドキュメント化）** を採用し、以下を定義する:

- SIGHUP 受信時はグレースフルシャットダウンを行う（SIGTERM と同等）
- config 変更時には daemon の完全再起動が必要であることをドキュメントに明記する

## Requirements

### Requirement 1: SIGHUP シグナルハンドリング

**Objective:** cupola daemon 運用者として、SIGHUP を受信した際にも daemon が即時終了せずグレースフルシャットダウンされることを望む。これにより、意図しない SIGHUP（端末切断など）による強制終了を防ぎ、実行中セッションを安全に終了させることができる。

#### Acceptance Criteria

1.1. When SIGHUP シグナルを受信した時、the cupola daemon shall グレースフルシャットダウン処理を開始し、ログにシグナル受信と対応を記録する
1.2. While polling loop が実行中に SIGHUP を受信した場合、the cupola daemon shall SIGTERM と同一のグレースフルシャットダウン手順を実行する（実行中セッションの kill → 最大 10 秒の完了待機）
1.3. The cupola daemon shall デフォルト動作（OS によるプロセス即時終了）をオーバーライドし、SIGHUP を明示的にハンドリングする
1.4. If SIGHUP 受信時に config reload を試みた場合、the cupola daemon shall reload は行わずにシャットダウンし、ログにその旨を記録する

### Requirement 2: ドキュメント更新

**Objective:** cupola 利用者として、SIGHUP の挙動と config 変更時の運用手順をドキュメントから確認できることを望む。

#### Acceptance Criteria

2.1. The documentation shall SIGHUP を受信した場合にグレースフルシャットダウンが実行される旨を記述する
2.2. The documentation shall config（`cupola.toml`）を変更した場合は daemon の再起動が必要である旨を明記する
2.3. The documentation shall SIGHUP による config reload は現時点では実装されておらず、将来的な実装を検討していることを記述する
