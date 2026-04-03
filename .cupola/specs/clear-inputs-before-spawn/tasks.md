# 実装タスク

## タスク一覧

- [ ] 1. inputs ディレクトリクリア関数の実装
- [ ] 1.1 `clear_inputs_dir` 関数を io.rs に追加する
  - `worktree_path/.cupola/inputs/` ディレクトリを `remove_dir_all` で削除し、`create_dir_all` で再作成する
  - `remove_dir_all` が `ErrorKind::NotFound` を返した場合は無視して続行する
  - それ以外のエラーは `with_context` でラップして `Result::Err` として返す
  - 関数シグネチャは `pub fn clear_inputs_dir(worktree_path: &Path) -> Result<()>`
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3_

- [ ] 2. prepare_inputs へのクリア処理の組み込み
- [ ] 2.1 `prepare_inputs` の先頭で `clear_inputs_dir` を呼び出す
  - `match issue.state { ... }` の直前に `clear_inputs_dir(wt)?` の1行を追加する
  - 既存のエラーハンドリング（`tracing::warn!` + `continue`）がクリア失敗もカバーすることを確認する
  - _Requirements: 1.1, 1.3, 1.4, 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 3. ユニットテストの追加
- [ ] 3.1 `clear_inputs_dir` のユニットテストを io.rs に追加する
  - ディレクトリとファイルが存在する場合: クリア後に空ディレクトリが残ることを検証
  - ディレクトリが存在しない場合: エラーなく完了し空ディレクトリが作成されることを検証
  - 連続呼び出し: 2回目も正常に完了することを検証
  - _Requirements: 1.1, 1.2, 2.1, 2.2_

- [ ]* 3.2 `prepare_inputs` の統合テストを追加する（任意）
  - 異なる State / causes の連続実行で残留ファイルが削除されることを検証
  - `State::DesignRunning` → `State::DesignFixing`（ReviewComments のみ）のシナリオ: `issue.md` が削除され `review_threads.json` のみ残ることを確認
  - `State::DesignFixing`（ReviewComments のみ）→ `State::DesignFixing`（CiFailure のみ）のシナリオ: `review_threads.json` が削除され `ci_errors.txt` のみ残ることを確認
  - _Requirements: 1.4, 2.1, 3.1, 3.2, 3.3, 3.4, 3.5_
