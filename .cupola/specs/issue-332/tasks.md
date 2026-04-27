# Implementation Plan

- [x] 問題が 1 件以上検出された場合に、利用者が修正後の再確認を行うべきことを示す案内が結果の末尾に表示される。
  _Requirements: FR-1 — 末尾ガイダンスは警告または失敗がある場合のみ表示され、問題がない場合は表示されないこと。_

- [x] Git に関する案内文がすべての該当ケースで統一され、利用者に同じ形式の remediation が提示される。
  _Requirements: FR-2 — Git 未設定時の案内文は全該当パスで同一の文面に揃っていること。_

- [x] GitHub CLI の未インストール案内文がフォーマットルールに従って統一される。
  _Requirements: FR-2 — `github token` チェックの gh 未インストールパスの案内文が統一フォーマットに準拠していること。_

- [x] Claude CLI に関する案内文がすべての該当ケースで統一される。
  _Requirements: FR-2 — `claude CLI` チェックの全エラーパスで同一フォーマットの案内文が使用されること。_

- [x] 環境変数許可リストに関する案内文がフォーマットルールに従って統一される。
  _Requirements: FR-2 — ワイルドカードパスおよび危険パターンパスの案内文で key/value がバッククォートで囲まれていること。_

- [x] ラベルチェックの gh 未インストールパスで案内文が統一フォーマットに準拠する。
  _Requirements: FR-2 — `agent:ready ラベル` / `weight:* ラベル` の gh 未インストール時の案内文が統一フォーマットに準拠していること。_

- [x] CI 修正上限通知チェックの案内文がフォーマットルールに従って統一される。
  _Requirements: FR-2 — Warn パスおよび DB エラーパスでコマンド名がバッククォートで囲まれていること。_

- [x] remediation 文字列を検証する既存テストが新フォーマットに追従して通過する。
  _Requirements: FR-3 — `cargo test` が全テスト通過すること。_

- [x] ビルドとコード品質チェックがエラーなしで完了する。
  _Requirements: NFR-1 — 既存の公開 API に変更がなく、`cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` がすべて通過すること。_
