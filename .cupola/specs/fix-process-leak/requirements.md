# 要件定義書

## はじめに

本仕様は、`polling_use_case.rs` に存在する2つのプロセス管理バグを修正するための要件を定義する。

1. **graceful_shutdown のループ判定バグ**: `collect_exited()` が空を返すと即 break するため、kill 直後にプロセスがまだ終了していない場合でもループを抜けてしまい、ゾンビプロセスが残る。
2. **recover_on_startup での孤児プロセス未処理**: 起動時に `current_pid` を DB からクリアするが、実プロセスの生存確認をしていない。前回のデーモンがクラッシュした場合、Claude Code プロセスが孤児として動作し続け、API コストが無駄に発生する。

## 要件

### 要件 1: graceful_shutdown のループ終了判定修正

**目的:** デーモン運用者として、シャットダウン時にすべての子プロセスが確実に終了した状態でプロセスを終了したい。それにより、ゾンビプロセスや API コストの漏洩を防ぎたい。

#### 受け入れ基準

1. When `graceful_shutdown` が呼ばれた後のウェイトループが実行されるとき、the Cupola daemon shall `collect_exited()` の戻り値ではなく `session_mgr.count() == 0` を終了条件として判定する。
2. When `kill_all()` 直後に `collect_exited()` が空リストを返しても、the Cupola daemon shall セッション数が 0 になるまでループを継続する。
3. While シャットダウンのデッドライン（10 秒）に達していない間、the Cupola daemon shall 500ms ごとに `collect_exited()` を呼び出してプロセス回収を試み続ける。
4. When デッドラインに達した場合、the Cupola daemon shall 強制 kill を実施し、警告ログを出力してループを終了する。
5. The Cupola daemon shall シャットダウン完了後に PID ファイルを削除する（既存の動作を維持する）。

### 要件 2: recover_on_startup での孤児プロセス kill 処理

**目的:** デーモン運用者として、前回クラッシュ時に残された孤児 Claude Code プロセスを起動時に自動的に終了させたい。それにより、無駄な API コスト発生を防ぎたい。

#### 受け入れ基準

1. When 起動時に `recover_on_startup` が実行され、Issue の `current_pid` が Some である場合、the Cupola daemon shall そのPIDのプロセスが生存しているか確認する。
2. If 指定 PID のプロセスが生存していると確認された場合、the Cupola daemon shall SIGKILL を送信してそのプロセスを終了させてから `current_pid` をクリアする。
3. If 指定 PID のプロセスが既に存在しない（dead）場合、the Cupola daemon shall プロセス kill を試みずに `current_pid` のみをクリアする（既存の動作を維持する）。
4. When プロセス kill に失敗した場合、the Cupola daemon shall 警告ログを出力し、`current_pid` のクリアは行う（起動処理を妨げない）。
5. The Cupola daemon shall プロセス生存確認に `is_process_alive(pid)` に相当するロジック（シグナル 0 送信）を使用する。

### 要件 3: テストカバレッジ

**目的:** 開発者として、両バグ修正に対して自動テストを追加したい。それにより、リグレッションを防ぎ、修正の正確性を保証したい。

#### 受け入れ基準

1. The テストスイート shall `graceful_shutdown` において `collect_exited()` が最初に空を返しても `count()` が 0 になった時点でループが終了することを検証するユニットテストを含む。
2. The テストスイート shall `recover_on_startup` において `current_pid` が Some かつプロセスが生存している場合に kill が呼ばれることを検証するユニットテストを含む。
3. The テストスイート shall `recover_on_startup` において `current_pid` が Some かつプロセスが既に終了している場合に kill が呼ばれないことを検証するユニットテストを含む。
4. The テストスイート shall `cargo test` で全テストがパスすることを確認する。
5. The テストスイート shall `cargo clippy -- -D warnings` で警告が発生しないことを確認する。
