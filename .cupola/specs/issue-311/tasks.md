# 実装計画

- [ ] 1. SessionEntry と ExitedSession の構造を変更する
- [ ] 1.1 SessionEntry から JoinHandle<String> フィールドを削除し、stdout_path / stderr_path を追加する
  - `stdout_handle: Option<JoinHandle<String>>` と `stderr_handle: Option<JoinHandle<String>>` を削除する
  - 代わりに `stdout_path: PathBuf` と `stderr_path: PathBuf` を追加する
  - オプションで `stdout_reader_handle: Option<JoinHandle<()>>` / `stderr_reader_handle: Option<JoinHandle<()>>` を追加してプロセス完了時の join を可能にする
  - _Requirements: 1.3_

- [ ] 1.2 ExitedSession の stdout/stderr フィールドをパスフィールドに置換する
  - `stdout: String` / `stderr: String` を削除する
  - `stdout_path: PathBuf` / `stderr_path: PathBuf` を追加する
  - _Requirements: 3.1_

- [ ] 2. register 関数をストリーム書き込み方式に書き換える
- [ ] 2.1 register シグネチャに run_id を追加し、ファイルパスを確定させる
  - `pub fn register(&mut self, issue_id: i64, state: State, child: Child, run_id: i64)` にシグネチャを変更する
  - `.cupola/logs/process-runs/` ディレクトリを `std::fs::create_dir_all` で作成する
  - `run-{run_id}-stdout.log` / `run-{run_id}-stderr.log` のパスを構築し `SessionEntry` に格納する
  - _Requirements: 1.5, 5.1, 5.4_

- [ ] 2.2 stdout/stderr リーダースレッドをストリーム書き込みに変更する
  - `std::thread::spawn` 内で `File::create(path)` → `LineWriter<BufWriter<File>>` を構築する
  - `std::io::copy(&mut BufReader::new(stream), &mut writer)` でストリームコピーする
  - `File::create` 失敗時は `tracing::error!(path = %path.display(), error = %e, ...)` を出力してスレッドを終了する（プロセスは継続）
  - スレッドの `JoinHandle<()>` を `SessionEntry` に格納し、`collect_exited` で join することでファイルの完全書き込みを保証する
  - _Requirements: 1.1, 1.2, 1.4, 2.1, 4.1, 4.4_

- [ ] 2.3 spawn_process から update_run_id 呼び出しを削除し、register に run_id を渡す
  - `execute.rs` の `spawn_process` 内で `session_mgr.register(issue.id, issue.state, child, run_id)` に変更する
  - `session_mgr.update_run_id(issue.id, run_id)` の呼び出しを削除する
  - _Requirements: 1.5_

- [ ] 3. resolve.rs をファイル読み取り方式に変更する
- [ ] 3.1 dump_session_io 関数を削除する
  - `fn dump_session_io(...)` 関数本体と、それを呼び出す `process_exited_session` 内の行を削除する
  - _Requirements: 3.4_

- [ ] 3.2 (P) stdout をファイルから読み取り、読み取り失敗時に mark_failed する
  - `process_exited_session` 内で `std::fs::read_to_string(&session.stdout_path)` を呼び出す
  - 失敗した場合は `mark_failed(run_id, format!("stdout log unavailable: {e}"))` を呼び出して早期リターンする
  - 成功した場合は従来の `parse_pr_creation_output(&stdout)` フローを継続する
  - _Requirements: 3.2, 4.2, 4.3_

- [ ] 3.3 (P) stderr スニペット生成をファイル読み取りに変更する
  - `process_exited_session` 内のプロセス失敗パスで `session.stderr.trim()` の代わりに `std::fs::read_to_string(&session.stderr_path)` を使用する
  - 読み取り失敗時は空文字列として扱い処理を継続する（stderr 欠落は致命的でない）
  - _Requirements: 3.3_

> 3.2 と 3.3 は同じ関数内だが修正箇所が独立しており並行作業可能（P）

- [ ] 4. ファイル書き込み失敗フォールバックをテストする
- [ ] 4.1 (P) リーダースレッドのファイル作成失敗ケースをテストする
  - `File::create` が失敗する条件（読み取り専用ディレクトリなど）でリーダースレッドが `tracing::error!` を出力してプロセスが継続することを検証する
  - _Requirements: 4.1, 4.4_

- [ ] 4.2 (P) resolve フェーズのファイル読み取り失敗ケースをテストする
  - `stdout_path` が存在しないファイルを指す `ExitedSession` で `process_exited_session` を呼ぶとき `mark_failed` が呼ばれることを検証する
  - _Requirements: 4.2, 4.3_

- [ ] 5. 既存テストをファイルパスベースに移行する
- [ ] 5.1 session_manager.rs のテストを更新する
  - `collect_exited_returns_finished_process` テストで `exited[0].stdout` の文字列比較を `std::fs::read_to_string(&exited[0].stdout_path)` による内容検証に変更する
  - `register` の引数が変わるため、全テスト内の `mgr.register(...)` 呼び出しに `run_id` を追加する
  - _Requirements: 6.1_

- [ ] 5.2 resolve.rs の make_session ヘルパーをファイルフィクスチャ化する
  - `make_session` ヘルパーを `TempDir` を受け取り、`stdout_path` / `stderr_path` にフィクスチャファイルを作成するように変更する
  - 既存の PR 作成・mark_failed・branch 検証テスト（T-2.1, T-2.2, T-2.3, T-2.base.1, T-2.base.2）がそのまま動作することを確認する
  - _Requirements: 6.2, 6.4_

- [ ] 5.3 (P) ストリーム書き込みの統合テストを追加する
  - `spawn_echo` など実プロセスを使ったテストで、プロセス実行中にログファイルが書き込まれることを検証する
  - `collect_exited` 後にファイル内容を読み取り、期待する出力が含まれることを確認する
  - _Requirements: 6.3_
