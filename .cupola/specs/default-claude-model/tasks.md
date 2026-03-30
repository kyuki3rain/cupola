# Implementation Plan

- [ ] 1. Config 値オブジェクトに model フィールドを追加する
  - Config 構造体に model フィールドを追加し、デフォルトモデル名を保持できるようにする
  - default_with_repo でデフォルト値 "sonnet" を設定する
  - 既存テストに model フィールドのアサーションを追加する
  - model が "sonnet" であることを検証するユニットテストを追加する
  - _Requirements: 2.1, 2.2_

- [ ] 2. CupolaToml パーサーと Config 変換に model 対応を追加する
- [ ] 2.1 (P) CupolaToml に model フィールドを追加し TOML パースに対応する
  - CupolaToml 構造体にオプショナルな model フィールドを追加する
  - model を含む TOML が正しくパースされることを検証するテストを追加する
  - model 未指定の TOML でも既存のパースが成功することを確認する
  - _Requirements: 1.1, 1.3_

- [ ] 2.2 (P) into_config で model の変換とデフォルト値適用を実装する
  - into_config メソッドで model 値を Config に反映する
  - model 未指定時にデフォルト値 "sonnet" を適用する
  - model 指定時に指定値が Config に反映されることを検証するテストを追加する
  - model 未指定時にデフォルト値が適用されることを検証するテストを追加する
  - _Requirements: 1.2, 2.3, 5.1_

- [ ] 3. ClaudeCodeRunner trait と ClaudeCodeProcess に model パラメータを追加する
- [ ] 3.1 (P) ClaudeCodeRunner trait の spawn メソッドに model 引数を追加する
  - trait 定義に model パラメータを追加する
  - コンパイラが全実装の更新を強制することを利用して、未対応箇所を検出する
  - _Requirements: 3.1, 3.2_

- [ ] 3.2 (P) ClaudeCodeProcess の build_command と spawn に model 対応を追加する
  - build_command メソッドに model パラメータを追加し、--model フラグをコマンド引数に含める
  - spawn メソッドから build_command に model を渡す
  - build_command が --model フラグと指定したモデル名を引数に含むことを検証するテストを追加する
  - 異なるモデル名（sonnet, opus）で正しく引数が生成されることを確認する
  - _Requirements: 4.1, 4.2, 4.3_

- [ ] 4. PollingUseCase から Config の model を spawn に渡すよう統合する
  - PollingUseCase の spawn 呼び出しで config.model を引数に追加する
  - 統合テスト内の mock ClaudeCodeRunner 実装を新しいシグネチャに更新する
  - 既存の全テストがパスすることを確認する
  - _Requirements: 5.1, 5.2_
