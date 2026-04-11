# 要件定義書

## はじめに

`Config::validate` 関数（`src/domain/config.rs`）は、一部の数値設定フィールドをバリデーションするが、`max_retries` フィールドのゼロ値チェックが欠如している。`max_retries = 0` が許容されると、`decide.rs` 内のリトライ判定ロジック（`consecutive_failures >= cfg.max_retries`）が初回失敗時に常に `0 >= 0` = `true` となり、すべての Issue が即座にキャンセルされる重大な誤動作を引き起こす。本変更は、この欠落したバリデーションを追加し、数値設定フィールドのバリデーション一貫性を確保することを目的とする。

## 要件

### 要件 1: max_retries フィールドのゼロ値バリデーション

**目的:** 設定管理者として、`max_retries = 0` の誤設定を起動時に検出したい。そうすることで、リトライ動作が意図せず無効化されてサービスが無音で壊れるのを防止できる。

#### 受け入れ基準

1. When `Config::validate` が呼び出されたとき、`max_retries` フィールドが `0` の場合、the Config validation shall `Err("max_retries must be greater than 0".to_string())` を返す
2. When `Config::validate` が呼び出されたとき、`max_retries` フィールドが `1` 以上の場合、the Config validation shall バリデーションを通過する（`Ok(())` を返す）
3. The Config validation shall デフォルト値（`max_retries = 3`）のままではバリデーションエラーを発生させない
4. If `max_retries` が `0` に設定されている場合、the Config validation shall エラーメッセージとして `"max_retries must be greater than 0"` を返す

### 要件 2: バリデーションのテストカバレッジ

**目的:** 開発者として、`max_retries` バリデーションの正常・異常パスを網羅するユニットテストが欲しい。そうすることで、将来の変更でリグレッションが発生した場合にすぐに検出できる。

#### 受け入れ基準

1. The test suite shall `max_retries = 0` のとき `validate()` がエラーを返すことを検証するテストケースを含む
2. The test suite shall `max_retries = 1`（最小有効値）のとき `validate()` が成功することを検証するテストケースを含む
3. The test suite shall デフォルト設定（`Config::default_with_repo`）が `max_retries` に関してバリデーションを通過することを検証するテストケースを含む
4. When ゼロ値バリデーションのテストが実行されたとき、the test shall エラーメッセージの文字列が `"max_retries must be greater than 0"` と完全一致することをアサートする
