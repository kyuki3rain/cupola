# 要件定義書

## プロジェクト概要 (Input)

`PostRetryExhaustedComment` エフェクトが GitHub コメントに表示する失敗回数が、リトライ上限判定ロジックで使われる「連続失敗数（consecutive failures）」と乖離している。ドキュメント（`docs/architecture/effects.md`）は連続失敗数を表示すると明記しているが、実装は全プロセスタイプを横断した累計失敗数を使っている。この乖離を解消してコードとドキュメントの一貫性を回復する。

---

## 要件

### 要件 1: コメント表示回数の正確性

**目標:** ユーザーとして、リトライ上限に到達したコメントに「どのプロセスタイプが何回連続して失敗したか」を正確に確認できるようにしたい。これにより、設定値 `max_retries` と表示回数が一致し、混乱を避けられる。

#### 受け入れ基準

1. When `PostRetryExhaustedComment` エフェクトが生成されるとき、the Cupola system shall 上限を超えた特定プロセスタイプの連続失敗数（`consecutive_failures`）をコメント本文の回数として使用する。
2. The Cupola system shall 全プロセスタイプを横断した累計失敗数（`count_total_failures`）をリトライ上限コメントの回数として使用しない。
3. When Init、Design、Impl いずれかのプロセスが `consecutive_failures >= max_retries` により上限に達したとき、the Cupola system shall そのプロセスタイプの `consecutive_failures` を `%{count}` に渡してコメントを投稿する。

### 要件 2: エフェクトへの情報伝達

**目標:** ドメイン設計者として、`PostRetryExhaustedComment` エフェクトに上限を超えたプロセスタイプと連続失敗数を埋め込み、実行フェーズが追加クエリなしに正確な値を取得できるようにしたい。

#### 受け入れ基準

1. The Cupola domain shall `PostRetryExhaustedComment` エフェクトのペイロードに `process_type: ProcessRunType` および `consecutive_failures: u32` フィールドを含める。
2. When `decide.rs` が `go_cancelled_retry_exhausted` を呼び出すとき、the Cupola domain shall 発火プロセスタイプと対応する `consecutive_failures` 値をエフェクトに渡す。
3. The Cupola system shall 実行フェーズ（`execute.rs`）において `PostRetryExhaustedComment` のペイロードから連続失敗数を読み取り、追加の DB クエリを不要とする。

### 要件 3: ドキュメントとコードの整合性

**目標:** 開発者として、`docs/architecture/effects.md` の記載内容が実装と一致していることを確認できるようにしたい。これにより、将来の実装者が誤った挙動を参照しなくて済む。

#### 受け入れ基準

1. The Cupola system shall `docs/architecture/effects.md` の `PostRetryExhaustedComment` に関する記述（連続失敗数）がコード実装と一致することを保証する。
2. If ドキュメントと実装に乖離がある場合、the Cupola system shall コード側を正しい挙動に合わせ（ドキュメントを基準とする）、ドキュメントを変更せずに済む。

### 要件 4: 既存テストおよび新規テストの整合性

**目標:** 開発者として、修正後の挙動が自動テストによって検証され、リグレッションが検出できるようにしたい。

#### 受け入れ基準

1. When `PostRetryExhaustedComment` エフェクト（新ペイロード付き）を実行するとき、the Cupola system shall `count` に連続失敗数が渡されていることをユニットテストで検証する。
2. The Cupola system shall `decide.rs` のテストが、各プロセスタイプ（Init / Design / Impl）で上限到達時に正しいプロセスタイプと連続失敗数がエフェクトに含まれることを確認する。
3. If `count_total_failures` 関数が `execute.rs` から削除された場合、the Cupola system shall その削除が CI（`cargo clippy -- -D warnings`）をパスする。
