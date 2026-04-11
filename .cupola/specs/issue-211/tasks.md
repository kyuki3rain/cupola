# 実装計画

- [ ] 1. max_retries ゼロ値バリデーションの追加
- [ ] 1.1 Config::validate に max_retries のゼロ値チェックを追加する
  - `src/domain/config.rs` の `max_ci_fix_cycles` チェック（L81–83）直後に `if self.max_retries == 0` ガードを挿入する
  - エラーメッセージは `"max_retries must be greater than 0"` とし、既存メッセージ形式と統一する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. max_retries バリデーションのユニットテスト追加
- [ ] 2.1 ゼロ値拒否テストの追加
  - `validate_rejects_zero_max_retries`: `max_retries = 0` のとき `validate()` が `Err` を返し、エラーメッセージが `"max_retries must be greater than 0"` に完全一致することを `assert_eq!` で検証する
  - `Config::default_with_repo("o", "r", "main")` を使い `max_retries` を上書きする既存テストパターンを踏襲する
  - _Requirements: 2.1, 2.4_

- [ ] 2.2 (P) 正の値受け入れテストの追加
  - `validate_accepts_positive_max_retries`: `max_retries = 1` のとき `validate()` が `Ok(())` を返すことを検証する
  - _Requirements: 2.2_

- [ ] 2.3 (P) デフォルト値受け入れテストの追加
  - `validate_accepts_default_max_retries`: `Config::default_with_repo` で生成した設定（`max_retries = 3`）が `validate()` を通過することを検証する
  - _Requirements: 2.3_
