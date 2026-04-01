# Implementation Plan

## Task Summary

- メジャータスク: 3
- サブタスク: 4
- 対応要件: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 3.1, 3.2, 3.3, 3.4

---

- [ ] 1. Markdown ドキュメントへの cc-sdd リンク追加
- [ ] 1.1 (P) README.md の cc-sdd 初出箇所にリンクを追加する
  - `README.md` L44 の `**cc-sdd (spec-driven development)**` を `**[cc-sdd](https://github.com/gotalab/cc-sdd) (spec-driven development)**` に変更する
  - 変更は L44 の1箇所のみであり、それ以降の cc-sdd 言及は変更しない
  - 周辺の太字記法 `**` および括弧がそのまま維持されていることを確認する
  - _Requirements: 1.1, 3.1, 3.2, 3.3_

- [ ] 1.2 (P) README.ja.md の cc-sdd 初出箇所にリンクを追加する
  - `README.ja.md` L44 の `**cc-sdd（仕様駆動開発）**` を `**[cc-sdd](https://github.com/gotalab/cc-sdd)（仕様駆動開発）**` に変更する
  - 変更は L44 の1箇所のみであり、それ以降の cc-sdd 言及は変更しない
  - 日本語の全角括弧（）がリンク記法の外側に維持されていることを確認する
  - _Requirements: 1.2, 3.1, 3.2, 3.3_

- [ ] 1.3 (P) CHANGELOG.md の cc-sdd 初出箇所にリンクを追加する
  - `CHANGELOG.md` L14 の `using cc-sdd` を `using [cc-sdd](https://github.com/gotalab/cc-sdd)` に変更する
  - Changelog エントリのインデントおよび文体を維持する
  - _Requirements: 1.3, 3.1, 3.2, 3.3_

- [ ] 1.4 (P) .cupola/steering/product.md の cc-sdd 初出箇所にリンクを追加する
  - `.cupola/steering/product.md` L3 の `Claude Code + cc-sdd to` を `Claude Code + [cc-sdd](https://github.com/gotalab/cc-sdd) to` に変更する
  - L3 が初出箇所であり、L7 の `using cc-sdd` は変更しない
  - 英文散文の文脈（前後の単語・スペース）を維持する
  - _Requirements: 1.4, 3.1, 3.2, 3.3_

- [ ] 2. YAML テンプレートへの cc-sdd URL 追記
  - `.github/ISSUE_TEMPLATE/cupola-task.yml` L10 の `cc-sdd の requirements フェーズ` を `cc-sdd (https://github.com/gotalab/cc-sdd) の requirements フェーズ` に変更する
  - YAML の `|` ブロックスカラー内のインデントを維持する
  - マークダウンリンク記法ではなくプレーンテキスト URL 形式を採用する（要件 2.2 に準拠）
  - ソースコードファイル（`.rs` ファイル等）には変更を加えない
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.4_

- [ ] 3. リンク追加内容の検証
  - 変更した全5ファイルで cc-sdd へのリンクまたは URL が正確に `https://github.com/gotalab/cc-sdd` を指していることを確認する
  - 各ファイルで cc-sdd へのリンク追加が初出箇所のみの1箇所であることを確認する
  - Markdown ファイルの構文が正しいこと（`[text](url)` 形式の閉じ忘れなし、太字記法の維持）を確認する
  - `cupola-task.yml` が有効な YAML 構文を維持していることを確認する
  - ソースコードファイルに意図しない変更がないことを確認する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_
