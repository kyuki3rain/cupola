# 要件定義

## はじめに

本仕様は、PIDファイルのフォーマット不一致およびstatusコマンドの出力ラベル誤りという3つのバグを修正する。ドキュメント仕様に準拠したPIDファイル2行フォーマットの実装、statusコマンドの正しいプレフィックス・モード表示・ラベルへの修正、レガシーPIDファイルへの後方互換性確保を目的とする。

## 要件

### 要件 1: PIDファイルの2行フォーマット対応

**目的:** 開発者として、起動モード（foreground/daemon）を含む2行フォーマットのPIDファイルを書き込み・読み込みできるようにしたい。これにより、statusコマンドが正確な起動モードを表示でき、ドキュメント仕様との整合性が保たれる。

#### 受け入れ基準

1. The system shall define a `ProcessMode` value type with `Foreground` and `Daemon` variants in the application port layer.
2. When the foreground process starts, the PID file management system shall write a 2-line PID file in the format `{pid}\n{mode}` where `{mode}` is `foreground`.
3. When the daemon child process starts, the PID file management system shall write a 2-line PID file in the format `{pid}\n{mode}` where `{mode}` is `daemon`.
4. When reading the PID file with mode, the PID file management system shall parse the first line as PID and the second line as mode, returning both values.
5. If the PID file contains only one line (legacy format), the PID file management system shall return `(pid, None)` and emit an `info`-level log message, without returning an error.
6. If the PID file does not exist, the PID file management system shall return `Ok(None)`.
7. If the PID file contains an invalid PID value, the PID file management system shall return `Err(PidFileError::InvalidContent(...))`.

### 要件 2: statusコマンドのプロセス状態表示修正

**目的:** 開発者として、statusコマンドがドキュメント仕様通りの `Process:` プレフィックスと起動モード表示を行うようにしたい。これにより、実装とドキュメントの乖離が解消され、ユーザーが正しい状態を把握できる。

#### 受け入れ基準

1. When the process is running and the mode is `foreground`, the status output system shall display `Process: running (foreground, pid={pid})`.
2. When the process is running and the mode is `daemon`, the status output system shall display `Process: running (daemon, pid={pid})`.
3. When the process is running and the mode is unknown (legacy single-line PID file), the status output system shall display `Process: running (unknown, pid={pid})`.
4. When the process is not running and no PID file exists, the status output system shall display `Process: not running`.
5. When a stale PID file is successfully cleaned up, the status output system shall display `Process: not running (stale PID file cleaned)`.
6. When a stale PID file cleanup fails, the status output system shall display `Process: not running (stale PID file exists, but cleanup failed)`.

### 要件 3: セッション数ラベルの修正

**目的:** 開発者として、statusコマンドのセッション数表示がドキュメント仕様通りの `Claude sessions:` ラベルを使用するようにしたい。

#### 受け入れ基準

1. The status output system shall display the session count as `Claude sessions: {alive}/{max}` when `max_concurrent_sessions` is configured, and as `Claude sessions: {alive}` when it is not configured.
