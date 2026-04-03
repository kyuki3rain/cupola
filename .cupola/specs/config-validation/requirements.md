# Requirements Document

## Introduction

Cupola は GitHub Issues と PRs をインターフェースとして動作する自動化エージェントであり、起動時に設定ファイルを読み込む。現状の `Config::validate()` は `max_concurrent_sessions` のゼロチェックのみを行っており、他のフィールドの不正値（空文字列・秒/分の取り違えなど）をそのまま通してしまう。本機能では、起動時に設定値の妥当性を厳格に検証し、不正な設定での起動を拒否することでユーザーのミスを早期に検出する。

## Requirements

### Requirement 1: 文字列フィールドの空文字チェック

**Objective:** Cupola の利用者として、`owner`、`repo`、`default_branch`、`language`、または `model` が空文字列のままで起動を試みた際に明確なエラーメッセージを受け取りたい。それにより、GitHub API が必ず失敗する状態やモデル指定なしでプロセスが起動し続けることを防ぎたい。

#### Acceptance Criteria

1. When `Config::validate()` が呼ばれ、`owner` フィールドが空文字列である, the Config shall `"owner must not be empty"` というエラーを返す。
2. When `Config::validate()` が呼ばれ、`repo` フィールドが空文字列である, the Config shall `"repo must not be empty"` というエラーを返す。
3. When `Config::validate()` が呼ばれ、`default_branch` フィールドが空文字列である, the Config shall `"default_branch must not be empty"` というエラーを返す。
4. When `Config::validate()` が呼ばれ、`language` フィールドが空文字列である, the Config shall `"language must not be empty"` というエラーを返す。
5. When `Config::validate()` が呼ばれ、`model` フィールドが空文字列である, the Config shall `"model must not be empty"` というエラーを返す。
6. When `Config::validate()` が呼ばれ、複数の文字列フィールドが空文字列である, the Config shall 定義順（owner → repo → default_branch → language → model）で最初に検出したフィールドのエラーを返す（早期リターン）。
7. When `Config::validate()` が呼ばれ、全ての文字列フィールド（owner、repo、default_branch、language、model）が非空文字列である, the Config shall このチェックに関してエラーを返さない。

---

### Requirement 2: polling_interval_secs の下限チェック

**Objective:** Cupola の利用者として、`polling_interval_secs` に秒/分の取り違えなどで極端に小さい値を設定した際にエラーを受け取りたい。それにより、CPU スピンと GitHub API レートリミット（5000回/時）の浪費を防ぎたい。

#### Acceptance Criteria

1. When `Config::validate()` が呼ばれ、`polling_interval_secs` が 10 未満である, the Config shall `"polling_interval_secs must be at least 10"` というエラーを返す。
2. When `Config::validate()` が呼ばれ、`polling_interval_secs` がちょうど 10 である, the Config shall このチェックに関してエラーを返さない。
3. When `Config::validate()` が呼ばれ、`polling_interval_secs` が 10 より大きい, the Config shall このチェックに関してエラーを返さない。
4. If `polling_interval_secs` が 0 である, the Config shall `"polling_interval_secs must be at least 10"` というエラーを返す（境界値: 0 は 10 未満）。

---

### Requirement 3: stall_timeout_secs の絶対下限チェック

**Objective:** Cupola の利用者として、`stall_timeout_secs` に秒/分の取り違えで極端に小さい値を設定した際にエラーを受け取りたい。それにより、Claude Code のプロセスが本来完了できるにもかかわらず即座に強制終了されることを防ぎたい。

#### Acceptance Criteria

1. When `Config::validate()` が呼ばれ、`stall_timeout_secs` が 60 未満である, the Config shall `"stall_timeout_secs must be at least 60"` というエラーを返す。
2. When `Config::validate()` が呼ばれ、`stall_timeout_secs` がちょうど 60 である, the Config shall このチェックに関してエラーを返さない（後続の比較チェックが通る場合）。
3. When `Config::validate()` が呼ばれ、`stall_timeout_secs` が 60 より大きい, the Config shall このチェックに関してエラーを返さない（後続の比較チェックが通る場合）。
4. If `stall_timeout_secs` が 30 である, the Config shall `"stall_timeout_secs must be at least 60"` というエラーを返す（秒/分取り違えの典型例）。

---

### Requirement 4: stall_timeout_secs と polling_interval_secs の相関チェック

**Objective:** Cupola の利用者として、`stall_timeout_secs` が `polling_interval_secs` 以下に設定された際にエラーを受け取りたい。それにより、stall 検出がポーリング1サイクル以内に発火してプロセスが即座に kill されることを防ぎたい。

#### Acceptance Criteria

1. When `Config::validate()` が呼ばれ、`stall_timeout_secs` が `polling_interval_secs` 以下である（かつ両フィールドが各絶対下限を満たす）, the Config shall `"stall_timeout_secs must be greater than polling_interval_secs"` というエラーを返す。
2. When `Config::validate()` が呼ばれ、`stall_timeout_secs` が `polling_interval_secs` と等しい, the Config shall `"stall_timeout_secs must be greater than polling_interval_secs"` というエラーを返す（境界値: 等値は NG）。
3. When `Config::validate()` が呼ばれ、`stall_timeout_secs` が `polling_interval_secs` より大きい, the Config shall このチェックに関してエラーを返さない。
4. The Config shall 絶対下限チェック（Requirement 2, 3）を相関チェック（Requirement 4）より先に評価し、絶対下限違反がある場合は相関チェックを実施しない。

---

### Requirement 5: 既存バリデーションとの共存

**Objective:** Cupola の利用者として、既存の `max_concurrent_sessions` バリデーションが引き続き機能することを期待する。それにより、バリデーション強化による既存動作の退行を防ぎたい。

#### Acceptance Criteria

1. The Config shall `max_concurrent_sessions` が `Some(0)` の場合に従来通りエラーを返す。
2. The Config shall 全ての新規バリデーションが既存の `max_concurrent_sessions` チェックと共存し、いずれかが失敗した場合にエラーを返す。
3. When 全てのバリデーション条件を満たす設定値が渡された, the Config shall `Ok(())` を返す。

---

### Requirement 6: log_dir の必須化

**Objective:** Cupola の利用者として、`log_dir` が空文字列のまま起動を試みた際に明確なエラーメッセージを受け取りたい。それにより、ログ出力先が未設定のままプロセスが動き続けることを防ぎたい。

#### Acceptance Criteria

1. When `Config::validate()` が呼ばれ、`log_dir` が空パス（空文字列から生成された `PathBuf`）である, the Config shall `"log_dir must not be empty"` というエラーを返す。
2. When `Config::validate()` が呼ばれ、`log_dir` が非空パスである, the Config shall このチェックに関してエラーを返さない。
3. The Config shall `log_dir` フィールドの型を `Option<PathBuf>` から `PathBuf` に変更し、デフォルト値を `".cupola/logs"` とする。
4. The Config shall TOML に `log.dir` が指定されていない場合、デフォルト値 `".cupola/logs"` を使用する。
5. The Config shall daemon 起動時に `log_dir` の `is_none()` チェックを不要とする（型変更により常に値が存在する）。
6. The Config shall `init_logging` の引数を `Option<&Path>` から `&Path` に変更し、常にファイルへのログ出力を行う。

---

### Requirement 7: エラーメッセージの品質

**Objective:** Cupola の利用者として、バリデーションエラー発生時に問題のフィールドと修正方法が明確に分かるメッセージを受け取りたい。それにより、設定ミスを迅速に修正できるようにしたい。

#### Acceptance Criteria

1. The Config shall 各バリデーションエラーメッセージにフィールド名を含める。
2. The Config shall 数値系バリデーションエラーメッセージに期待する下限値を含める。
3. The Config shall エラーメッセージを英語で返す（ログおよびエラー文字列は英語が標準）。
