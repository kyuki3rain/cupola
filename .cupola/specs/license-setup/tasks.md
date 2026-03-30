# Implementation Plan

- [x] 1. (P) LICENSE ファイルの作成
  - Apache License, Version 2.0 の標準テキスト全文をリポジトリルートに `LICENSE` ファイルとして配置する
  - 著作権行に `Copyright 2026 cupola contributors` を記載する
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. (P) README.md のライセンスセクション更新
  - 既存の `## License` セクションの TBD テキストを Apache License 2.0 の情報に置換する
  - ライセンス名を明記し、LICENSE ファイルへの参照リンクを含める
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 3. (P) Cargo.toml の license フィールド設定
  - `[package]` セクションの `edition` フィールド直後に `license = "Apache-2.0"` を追加する
  - `cargo metadata` でライセンス情報が正しく出力されることを確認する
  - _Requirements: 3.1, 3.2_
