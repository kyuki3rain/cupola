# 実装計画

- [ ] 1. domain 層に env whitelist value object を実装する
- [ ] 1.1 env パターンマッチのロジックを実装する
  - BASE_ALLOWLIST（HOME、PATH、USER、LANG、LC_ALL、TERM）を定数として定義する
  - サフィックス `*` ワイルドカードに対応したパターンマッチ関数を実装する
  - env whitelist 設定のデータ構造を定義し、デフォルト値として空の allowlist を使用する
  - ユニットテストを作成する: 完全一致・サフィックス wildcard マッチ・サフィックス wildcard 非マッチ・中間 wildcard（リテラル扱い）
  - _Requirements: 1.2, 1.3, 3.1, 3.2, 3.3_

- [ ] 1.2 (P) cupola 設定モデルに env allowlist フィールドを追加する
  - cupola の設定構造に env allowlist 設定を保持するフィールドを追加する
  - デフォルト値として空の allowlist が使われることを確認する
  - 既存テストが引き続きパスすることを確認する
  - _Requirements: 2.1_

- [ ] 2. cupola.toml の [claude_code.env] セクションを解析できるようにする
- [ ] 2.1 [claude_code.env] TOML セクションのパース構造を実装する
  - 既存の他セクション（[models]、[log] 等）と同様のパターンで [claude_code.env] セクションの解析構造を定義する
  - toml 解析対象の設定データに [claude_code.env] エントリを追加する
  - セクション未設定時は extra_allow が空リストとなるデフォルト動作を実装する
  - _Requirements: 2.1, 2.3, 2.4_

- [ ] 2.2 (P) TOML パースの統合テストを追加する
  - `[claude_code.env]` セクションを含む cupola.toml の解析が正しく機能することを確認する
  - セクション未設定時に `extra_allow` が空リストになることを確認するテストを追加する
  - _Requirements: 2.1, 2.3, 2.4_

- [ ] 3. Claude Code プロセス起動時に env 制限を適用する
- [ ] 3.1 Claude Code プロセスに env allowlist 設定を受け取り適用する機能を実装する
  - Claude Code プロセス生成時に env allowlist 設定を受け取れるようにする
  - プロセスコマンド構築時に必ず env_clear() を適用し全 env 継承を廃止する
  - BASE_ALLOWLIST に含まれる env var のみを親プロセスから継承する
  - extra_allow のパターンにマッチする env var を親プロセスから継承する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 3.1, 3.2_

- [ ] 3.2 (P) Claude Code プロセスの env 制限に関するユニットテスト・統合テストを追加する
  - env_clear が適用され、BASE_ALLOWLIST 以外の env var がコマンドに含まれないことを確認する
  - BASE_ALLOWLIST の env var が正しく引き継がれることを確認する
  - exact match パターンが正しく機能することを確認する
  - ワイルドカードパターンが正しく機能することを確認する
  - マッチしない env var が除外されることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 3.1, 3.2_

- [ ] 3.3 プロセス起動の依存性注入箇所を更新する
  - Claude Code プロセスを生成する DI 箇所で env allowlist 設定を注入するよう更新する
  - _Requirements: 1.1, 2.2_

- [ ] 4. (P) cupola init テンプレートに [claude_code.env] セクションを追加する
  - init コマンドで生成される cupola.toml テンプレートに [claude_code.env] セクションをコメントアウト状態で追加する
  - 代表的な extra_allow 候補（ANTHROPIC_API_KEY、CLAUDE_*、OPENAI_API_KEY、DOCKER_HOST）をコメント例として含める
  - ワイルドカードサポートの説明コメントを追加する
  - _Requirements: 4.1, 4.2_

- [ ] 5. doctor コマンドに env allowlist チェックを追加する
- [ ] 5.1 doctor の設定サマリーに env allowlist 情報を追加する
  - doctor が返す設定サマリーに extra_allow パターン一覧を含めるよう拡張する
  - config ロード成功時のみ extra_allow が設定される（失敗時は env チェックがスキップされる）
  - 既存テストヘルパーのデフォルト値を更新して新フィールドに対応させる
  - _Requirements: 5.1, 5.4_

- [ ] 5.2 doctor に env allowlist の危険パターンチェックを追加する
  - env allowlist と危険パターンを照合するチェック関数を実装する
  - 問題がない場合: BASE_ALLOWLIST と extra_allow パターン一覧を Ok メッセージに含める
  - 完全一致・プレフィックス一致のパターン（GH_TOKEN、GITHUB_TOKEN、AWS_* 等）は `matches_pattern` で照合する
  - サフィックス一致のパターン（_API_KEY、_SECRET、_TOKEN、_PASSWORD）は `ends_with` で照合する（先頭 `*` ワイルドカードは `matches_pattern` では非対応のため）
  - いずれかにマッチする extra_allow エントリがあれば Warn を返す
  - Warn メッセージに該当パターン名と削除の推奨メッセージを含める
  - config ロード成功時の StartReadiness セクションにこのチェック結果を追加する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 5.3 (P) doctor env allowlist チェックのユニットテストを追加する
  - extra_allow が空のとき Ok を返すことを確認する
  - 安全なパターンのみのとき Ok を返すことを確認する
  - 完全一致パターン（GH_TOKEN 等）が含まれるとき Warn を返すことを確認する
  - プレフィックスパターン（AWS_* 等）が含まれるとき Warn を返すことを確認する
  - サフィックスマッチ（_API_KEY 等）が含まれるとき Warn を返すことを確認する
  - doctor の StartReadiness セクションにこのチェック結果が含まれることを確認する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_
