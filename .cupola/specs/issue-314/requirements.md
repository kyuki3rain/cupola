# 要件定義書

## はじめに

本ドキュメントは Issue #314「panic 時に子プロセスを回収する graceful exit (panic hook)」の要件を定義する。

デーモン内で panic が発生した際、子プロセス（Claude Code）が孤児化する問題を **panic hook ベースの graceful exit** で解決する。現在の `install_panic_hook` は PID ファイル削除とログ記録を行っているが、子プロセスのシャットダウンが欠落している。本機能では以下を保証する：

1. 子プロセス PID を panic hook から安全に参照できる共有レジストリを導入する
2. panic hook 内で子プロセスへ SIGTERM → タイムアウト後 SIGKILL を送る同期シャットダウンを実行する
3. panic の発生場所とメッセージを確実にログに残す
4. PID ファイルを削除してデーモンの再起動を妨げない

Issue #309 (Mutex 毒化対策) を本 issue に統合してクローズする。

## 要件

### 要件 1: 子プロセス PID 共有レジストリ

**目的**: As a cupola デーモン, I want SessionManager と panic hook が同じ子プロセス PID セットを参照できること, so that panic が起きても登録済みの全子プロセスに確実にシャットダウン信号を送れる。

#### 受け入れ条件

1. The system shall アクティブな子プロセス PID の共有レジストリを管理し、非同期ポーリングループと同期 panic hook の両方からアクセスできること。
2. When 子プロセスが `SessionManager::register` 経由で登録されたとき, the system shall その子プロセスの PID を共有レジストリに追加すること。
3. When 子プロセスが終了した場合（`collect_exited` で検出）またはシグナルで終了された場合（`kill` または `kill_all` 経由）, the system shall その子プロセスの PID を共有レジストリから削除すること。
4. The system shall スレッド境界を越えた安全な並行アクセスを保証する実装でレジストリを実現すること（`Arc<Mutex<HashSet<u32>>>` 等）。
5. If `SessionManager` がレジストリなしで設定された場合, the system shall これまでどおり動作すること（レジストリ統合はビルダーパターンによりオプトイン）。

### 要件 2: 同期シャットダウン API

**目的**: As a cupola デーモン, I want panic hook（同期コンテキスト）から子プロセスを確実に終了させるための API があること, so that tokio runtime が停止していても全子プロセスを回収できる。

#### 受け入れ条件

1. When 同期シャットダウン API が呼び出されたとき, the system shall 共有レジストリ内の全 PID に SIGTERM を送信すること。
2. When SIGTERM が送信されたとき, the system shall 全プロセスが終了するか SIGTERM タイムアウト（3 秒）が経過するまで、短い間隔で各プロセスの生存確認をポーリングすること。
3. If SIGTERM タイムアウト経過後も終了していない子プロセスが存在する場合, the system shall そのプロセスに SIGKILL を送信すること。
4. The system shall async/await コンテキストを必要とせずにシャットダウンシーケンス全体を実行すること（同期専用 API）。
5. If 呼び出し時点でレジストリの Mutex が毒化している場合, the system shall 内部値を回収してシャットダウンを続行すること（中断しないこと）。

### 要件 3: panic hook の拡張

**目的**: As a cupola オペレーター, I want デーモンが panic した際に子プロセスの自動回収・PID ファイル削除・ログ記録がすべて実行されること, so that ユーザーが手動で孤児プロセスをクリーンアップしなくて済む。

#### 受け入れ条件

1. When デーモンが panic したとき, the system shall 他のクリーンアップより前に同期シャットダウン API を呼び出して登録済み子プロセスを全て終了させること。
2. When デーモンが panic したとき, the system shall PID ファイルを削除すること（ベストエフォート：エラーは無視）。
3. When デーモンが panic したとき, the system shall パニックメッセージとファイル・行番号をエラーログに記録すること。
4. When 全クリーンアップが完了したとき, the system shall 標準パニック出力（stderr バックトレース / abort）を維持するために直前のフック（デフォルトフック）を呼び出すこと。
5. The system shall 子プロセスのシャットダウンを PID ファイル削除より前に実行すること。シャットダウンシーケンスは「子プロセス終了 → PID 削除 → デフォルトフック」の順であること。

### 要件 4: bootstrap 配線

**目的**: As a cupola 開発者, I want bootstrap 起動時に共有レジストリが正しく panic hook と SessionManager の両方に接続されること, so that 実際の panic 経路で全コンポーネントが協調して動作する。

#### 受け入れ条件

1. When フォアグラウンドまたはデーモン子プロセスモードで起動するとき, the system shall `ChildProcessRegistry` を panic hook のインストールより前に作成すること。
2. The system shall レジストリのクローンを一つ `install_panic_hook` に渡し、もう一つを `PollingUseCase`（`SessionManager` へ転送）に渡すこと。
3. `PollingUseCase` shall `with_process_repo` や `with_pid_file` と同様のビルダーメソッドを公開し、レジストリを `SessionManager` に接続できるようにすること。
4. The system shall `start_foreground` と `start_daemon_child` の両方に子プロセスシャットダウン機能付きの拡張 panic hook をインストールすること。

### 要件 5: 統合テスト

**目的**: As a cupola 開発者, I want panic シナリオで孤児プロセスが残らないことを自動テストで検証できること, so that 将来の変更でリグレッションが発生しても即座に検出できる。

#### 受け入れ条件

1. When Mutex 毒化パニックがシミュレートされたとき（`SqliteConnection::conn_lock` の毒化を模倣）, the integration test shall フック実行後にレジストリ内の子プロセス PID が残存していないことを検証すること。
2. When `spawn_blocking` パニックがシミュレートされたとき, the integration test shall フック実行後に登録済み子プロセス PID が残存していないことを検証すること。
3. When panic hook の実行が完了したとき, the integration test shall PID ファイルが削除されていることを検証すること。
4. The integration test shall CI での不安定テストを防ぐために、合理的な時間制限（SIGTERM タイムアウト＋マージン）以内に完了すること。
5. Where 既存の `kill_all` 動作がテストされている場合, the system shall `kill` および `kill_all` 操作後にレジストリエントリが適切に削除されることを確認するリグレッションテストを追加すること。
