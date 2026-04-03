# 要件定義書

## プロジェクト説明（入力）
fix: spawn前に.cupola/inputs/をクリアする - prepare_inputsの冒頭でinputsディレクトリを削除・再作成し、前回のFixingで書かれた古いファイルが次のspawnに影響しないようにする

## はじめに

本機能は、Cupola の `PollingUseCase` における `prepare_inputs` の冒頭で `.cupola/inputs/` ディレクトリを削除・再作成することで、前回の Fixing フェーズで書き込まれた古いファイルが次の spawn 実行時に Claude Code へ誤読されるバグを修正する。

`prepare_inputs` は `fixing_causes` に含まれる種類のファイルのみを書き込む設計になっているが、前回の実行で作成されたファイル（例: `review_threads.json`）がディレクトリに残存したまま次の spawn が走ると、Claude Code は無関係なファイルを読み込んで不正確なレスポンスを生成する。

## 要件

### 要件 1: spawn前のinputsディレクトリクリア

**目的:** Cupola の PollingUseCase 開発者として、各 spawn 実行前に `.cupola/inputs/` ディレクトリをクリーンな状態にしたい。そうすることで、前回の Fixing で残留した古いファイルが Claude Code の動作に干渉しなくなる。

#### 受け入れ基準

1. When `prepare_inputs` が呼ばれたとき、the PollingUseCase shall `.cupola/inputs/` ディレクトリ（`worktree_path.join(".cupola/inputs")` のパス）内の全ファイルを削除してから新規ファイルの書き込みを行う。
2. When `.cupola/inputs/` ディレクトリが存在しない状態で `prepare_inputs` が呼ばれたとき、the PollingUseCase shall ディレクトリを新規作成してファイル書き込みを正常に完了する。
3. When `.cupola/inputs/` ディレクトリのクリアに失敗したとき、the PollingUseCase shall エラーログを出力してエラーを呼び出し元に伝播する。
4. The PollingUseCase shall クリア後に `prepare_inputs` が書き込む対象ファイルのみが `.cupola/inputs/` に存在することを保証する。

### 要件 2: クリア処理のべき等性と安全性

**目的:** Cupola の PollingUseCase 開発者として、ディレクトリクリア処理が冪等かつ安全に動作してほしい。そうすることで、ポーリングの再実行や予期しない状態でもシステムが安定する。

#### 受け入れ基準

1. When `prepare_inputs` が同一 Issue に対して複数回呼ばれたとき、the PollingUseCase shall 毎回ディレクトリをクリアしてから現在の `fixing_causes` に対応するファイルのみを書き込む。
2. While `.cupola/inputs/` ディレクトリが空のとき、the PollingUseCase shall クリア処理を正常に完了しエラーを発生させない。
3. The PollingUseCase shall クリア対象を worktree 配下の `.cupola/inputs/` ディレクトリのみに限定し、その配下は再帰的に削除して再作成してよいが、worktree 外のファイルやディレクトリを削除しない。

### 要件 3: 既存の書き込みロジックとの一貫性

**目的:** Cupola の PollingUseCase 開発者として、クリア処理の追加が既存の入力ファイル書き込みロジックに影響を与えないことを保証したい。そうすることで、各 State に応じた正しいファイルが引き続き生成される。

#### 受け入れ基準

1. When `State::DesignRunning` で `prepare_inputs` が呼ばれたとき、the PollingUseCase shall クリア後に `issue.md` のみを書き込む。
2. When `State::DesignFixing` または `State::ImplementationFixing` で `fixing_causes` に `ReviewComments` が含まれるとき、the PollingUseCase shall クリア後に `review_threads.json` を書き込む。
3. When `State::DesignFixing` または `State::ImplementationFixing` で `fixing_causes` に `CiFailure` が含まれるとき、the PollingUseCase shall クリア後に `ci_errors.txt` を書き込む。
4. When `State::DesignFixing` または `State::ImplementationFixing` で `fixing_causes` に `Conflict` が含まれるとき、the PollingUseCase shall クリア後に `conflict_info.txt` を書き込む。
5. If `fixing_causes` が空のとき（後方互換フォールバック）、the PollingUseCase shall クリア後に `review_threads.json` を書き込む。
