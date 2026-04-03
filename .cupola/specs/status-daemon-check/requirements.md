# 要件定義書

## プロジェクト説明（入力）

statusコマンドにdaemon起動状態とプロセス生存確認を追加する機能

## はじめに

Cupolaの `status` コマンドは現在、DBの `current_pid` が `Some` であれば「Running」としてカウントしているが、daemonプロセスが停止していてもPIDがDBに残っていれば「Running」と表示される問題がある。本機能では、PIDファイルを用いたdaemon起動状態の表示と、各issueのプロセス生存確認（signal 0チェック）を追加することで、実態に即した状態表示を実現する。

## 要件

### 要件 1: Daemon起動状態の表示

**目的：** オペレーターとして、statusコマンドでdaemonが実際に起動しているかどうかを確認したい。それにより、PIDがDBに残っているだけの停止済みdaemonを誤って「Running」と判断するリスクをなくしたい。

#### 受け入れ基準

1. When `cupola status` が実行され、PIDファイルが存在し、当該PIDのプロセスが生存している場合、the Cupola CLI shall `Daemon: running (pid=<PID>)` の形式でdaemon状態を出力する。
2. When `cupola status` が実行され、PIDファイルが存在しない場合、the Cupola CLI shall `Daemon: not running` と出力する。
3. When `cupola status` が実行され、PIDファイルが存在するが当該PIDのプロセスが存在しない場合、the Cupola CLI shall stale PIDファイルを削除した上で `Daemon: not running (stale PID file cleaned)` と出力する。
4. The Cupola CLI shall daemon状態をissue一覧より先に出力する。
5. The Cupola CLI shall 既存の `PidFileManager::is_process_alive` を再利用してプロセス生存確認を行う。

### 要件 2: Issue ごとのプロセス生存確認

**目的：** オペレーターとして、各issueに紐づくプロセスが実際に動いているかどうかをstatusコマンドで確認したい。それにより、DBに残っている古いPIDを持つissueを「Running」と誤表示することを防ぎたい。

#### 受け入れ基準

1. When `cupola status` が実行され、issueの `current_pid` が `Some(pid)` であり、当該プロセスが生存している場合、the Cupola CLI shall issueの行末に `pid:<PID> (alive)` を付加して表示する。
2. When `cupola status` が実行され、issueの `current_pid` が `Some(pid)` であり、当該プロセスが存在しない場合、the Cupola CLI shall issueの行末に `pid:<PID> (dead)` を付加して表示する。
3. When `cupola status` が実行され、issueの `current_pid` が `None` の場合、the Cupola CLI shall PID情報を付加せずに既存の形式で表示する。
4. The Cupola CLI shall Running: のカウントを、`current_pid` の有無だけでなく、プロセスが実際に生存しているissueの数に基づいて算出する。

### 要件 3: 既存表示の維持

**目的：** オペレーターとして、既存のstatusコマンドで表示されていた情報（state、PR番号、retry_count、worktree_path）が引き続き表示されることを期待する。

#### 受け入れ基準

1. The Cupola CLI shall issue一覧において `github_issue_number`、`state`、`design_pr_number`、`impl_pr_number`、`retry_count`、`worktree_path` を従来通りに出力する。
2. The Cupola CLI shall `error_message` が存在する場合、従来通りにエラー情報を付加して出力する。
3. The Cupola CLI shall DBが存在しない場合のエラーメッセージ（`No database found. Run cupola init first.`）を維持する。
4. While アクティブなissueが存在しない場合、the Cupola CLI shall `No active issues.` と出力する。
