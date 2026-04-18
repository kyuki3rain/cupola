# Requirements Document

## Project Description (Input)
## 背景

現在の shutdown は即 SIGKILL でプロセスを殺すが、将来的にはエージェントプロセスの完了を待つモードが欲しい。実行中のタスクが長時間かかる場合でも、途中で殺さず完了まで待てると API コストの無駄を防げる。

## 検討事項

- **SIGTERM → wait → SIGKILL の2段階 shutdown**: SIGTERM で「新規タスクを受けず、現在のタスクを完了して終了」を指示し、タイムアウト後に SIGKILL
- **2回 Ctrl+C で強制 kill**: 1回目は graceful wait、2回目で即座に SIGKILL
- **`stop` コマンドのオプション**: `stop --force` で即 SIGKILL、`stop`（デフォルト）で graceful wait
- **タイムアウトの設定化**: `shutdown_timeout_secs` を `cupola.toml` で設定可能にする。`0` または `"infinite"` で無限待機（完了するまで終了しない）も設定できること

## 関連

- #163 (現状の shutdown バグ修正が前提)

## Requirements

### Requirement 1: graceful shutdown モード（SIGTERM → 待機 → SIGKILL）

**Objective:** Cupola オペレーターとして、実行中のエージェントセッションが完了するまで shutdown を待機させたい。そうすることで API コストの無駄遣いを防げる。

#### Acceptance Criteria

1. When SIGTERM を受信した, the Cupola daemon shall 新規セッションの受け付けを停止し、現在実行中のセッションの完了を待機してから終了する
2. While graceful shutdown 待機中, the Cupola daemon shall 新しい Issue のポーリングおよびセッション起動を行わない
3. When 設定済みの shutdown_timeout_secs が経過した, the Cupola daemon shall 残存するすべてのセッションに SIGKILL を送信して強制終了する
4. When すべての実行中セッションが正常に完了した, the Cupola daemon shall PID ファイルを削除してゼロ終了コードでプロセスを終了する
5. The Cupola daemon shall graceful shutdown 待機中に定期的に「完了待ち中 (残 N セッション)」のログを出力する

### Requirement 2: 強制 shutdown モード（即時 SIGKILL）

**Objective:** Cupola オペレーターとして、緊急時または意図的に即座にプロセスを終了させたい。

#### Acceptance Criteria

1. When `cupola stop --force` コマンドを実行した, the Cupola CLI shall 対象プロセスに即座に SIGKILL を送信する
2. When フォアグラウンド実行中に Ctrl+C を2回連続して入力した, the Cupola daemon shall 即座に SIGKILL 相当の強制終了を実行する
3. When 強制終了を実行した, the Cupola daemon shall セッションの完了を待機せず PID ファイルを削除してプロセスを終了する
4. If 強制終了信号が受信された, the Cupola CLI shall 実行中セッション数が 1 以上のとき、強制終了を実行した旨と終了したセッション数をユーザーに表示する

### Requirement 3: shutdown タイムアウト設定

**Objective:** Cupola オペレーターとして、graceful shutdown の待機時間を用途に応じて調整したい。

#### Acceptance Criteria

1. The Cupola daemon shall `cupola.toml` の `shutdown_timeout_secs` フィールドから graceful shutdown のタイムアウト値を読み込む
2. Where `shutdown_timeout_secs` が `0` に設定されている, the Cupola daemon shall タイムアウトなしで全セッション完了まで無限に待機する
3. Where `shutdown_timeout_secs` が正の整数に設定されている, the Cupola daemon shall 指定秒数後に SIGKILL を発行してタイムアウトとする
4. If `cupola.toml` に `shutdown_timeout_secs` が未設定の場合, the Cupola daemon shall デフォルト値 300 秒でタイムアウトを適用する
5. The Cupola daemon shall `shutdown_timeout_secs` の変更を次回起動時に反映する

### Requirement 4: stop コマンドの --force オプション

**Objective:** Cupola オペレーターとして、`stop` コマンドから graceful／force の2通りの停止モードを明示的に選択できる CLI インターフェースが欲しい。

#### Acceptance Criteria

1. The Cupola CLI shall `stop` サブコマンドに `--force` フラグを追加する
2. When `cupola stop` を `--force` なしで実行した, the Cupola CLI shall graceful shutdown リクエストとして SIGTERM を送信する
3. When `cupola stop --force` を実行した, the Cupola CLI shall 強制終了リクエストとして SIGKILL を送信する（または SIGTERM 後に即時 SIGKILL フォールバック）
4. If `--force` を指定していないとき stop コマンドのタイムアウト（StopUseCase 側）が設定 shutdown_timeout_secs より著しく短い場合, the Cupola CLI shall 設定値に準拠した待機を行う

### Requirement 5: graceful shutdown 中の可視性

**Objective:** Cupola オペレーターとして、shutdown の進行状況をリアルタイムで把握したい。

#### Acceptance Criteria

1. While graceful shutdown 待機中, the Cupola daemon shall `cupola stop` 呼び出し元の端末へ待機状況（残セッション数、経過秒数）を定期的に出力する
2. When graceful shutdown がタイムアウトによる SIGKILL で完了した, the Cupola daemon shall タイムアウトで強制終了したことをログに記録する
3. When graceful shutdown が全セッション完了で正常終了した, the Cupola daemon shall 完了セッション数をログに記録して正常終了する
4. If graceful shutdown 中に追加の SIGTERM を受信した, the Cupola daemon shall 既に shutdown 待機中であることを示すログを出力し、タイマーをリセットしない
