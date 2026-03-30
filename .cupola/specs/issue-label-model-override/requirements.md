# 要件定義書

## はじめに

本機能は、GitHub Issue に付与された `model:*` ラベル（例: `model:opus`、`model:haiku`）を読み取り、その Issue を処理する Claude Code 起動時に使用するモデルを動的に上書きする機能を提供する。ラベルの変更は毎ポーリングサイクルで即座に検知・反映され、失敗後に人間がラベルを変更することで次のリトライから別モデルで再実行できる。

## 要件

### Requirement 1: model:* ラベルの検出と DB 記録

**Objective:** 開発者として、Issue に `model:*` ラベルを付与することで、その Issue の処理に使用する Claude モデルを指定できるようにしたい。それにより、Issue の複雑さに応じて適切なモデルを使い分けられる。

#### Acceptance Criteria

1. When Cupola が `agent:ready` ラベル付き Issue を検出するとき、the Cupola shall `model:*` パターンのラベルを解析し、抽出したモデル名を issues テーブルの `model` カラムに保存する。
2. If `model:*` パターンのラベルが Issue に存在しない場合、the Cupola shall issues テーブルの `model` カラムを `NULL` に設定する。
3. If `model:*` ラベルが複数存在する場合、the Cupola shall ラベルリストの先頭に見つかったものを優先して使用する。
4. The Cupola shall `model` カラムとして `nullable TEXT` 型を持つよう issues テーブルのスキーマを定義する。

---

### Requirement 2: ポーリングサイクルごとのラベル動的更新

**Objective:** 開発者として、Issue 処理中にラベルを変更した場合でも次のポーリングサイクルから即座に反映されるようにしたい。それにより、失敗した Issue に `model:opus` ラベルを後付けして高性能モデルで再実行できる。

#### Acceptance Criteria

1. While Issue が非終端状態（処理中）である間、When ポーリングサイクルが実行されるとき、the Cupola shall 対象 Issue の最新ラベル一覧を GitHub API から取得し、`model:*` ラベルを再解析する。
2. When `model:*` ラベルが追加・変更・削除されたとき、the Cupola shall issues テーブルの `model` カラムを最新の値（またはラベル削除時は `NULL`）に更新する。
3. When `model:*` ラベルが削除されたとき、the Cupola shall issues テーブルの `model` カラムを `NULL` に更新する。
4. The Cupola shall ラベルの再取得を既存の非終端 Issue に対するポーリング処理の Step 1（Issue 状態確認）内で実施する。

---

### Requirement 3: Claude Code 起動時のモデル優先順位制御

**Objective:** 開発者として、Issue レベル・設定ファイルレベル・デフォルトの優先順位でモデルが決定されるようにしたい。それにより、細かい制御と合理的なデフォルト動作が両立できる。

#### Acceptance Criteria

1. When Claude Code を spawn するとき、the Cupola shall 以下の優先順位でモデルを決定する：(1) issues テーブルの `model` カラム（非 NULL の場合）、(2) `cupola.toml` の `model` 設定値（設定されている場合）、(3) デフォルト値 `"sonnet"`。
2. When issues テーブルの `model` カラムが非 NULL のとき、the Cupola shall `--model <model>` フラグを付与して Claude Code を起動する。
3. If issues テーブルの `model` カラムが `NULL` であり `cupola.toml` の `model` が設定されている場合、the Cupola shall `cupola.toml` の値を `--model` フラグとして使用する。
4. If issues テーブルの `model` カラムも `cupola.toml` の `model` も未設定の場合、the Cupola shall `--model sonnet` を使用する。
5. The Cupola shall `ClaudeCodeRunner` トレイトの spawn インタフェースでモデル名を受け取れるようにする。

---

### Requirement 4: 後方互換性と既存テストの維持

**Objective:** 開発者として、既存機能が変更後も正常に動作することを保証したい。それにより、機能追加がデグレを引き起こさないことを確認できる。

#### Acceptance Criteria

1. The Cupola shall `model` カラムを `nullable TEXT` として追加することで、既存の issues テーブルレコードとの互換性を維持する（`NULL` は `model` 未指定を意味する）。
2. The Cupola shall 既存の全ユニットテストおよびインテグレーションテストがパスする状態を維持する。
3. When `model:*` ラベルなしで既存の Issue が処理されるとき、the Cupola shall 従来と同じ動作（`cupola.toml` のデフォルトまたは `"sonnet"`）を継続する。
