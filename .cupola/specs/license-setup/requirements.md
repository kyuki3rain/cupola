# Requirements Document

## Introduction
Apache License 2.0 の LICENSE ファイルをリポジトリルートに配置し、README.md のライセンスセクションおよび Cargo.toml の license フィールドを更新することで、OSS としてのライセンス明示を完了する。

## Requirements

### Requirement 1: LICENSE ファイルの作成
**Objective:** As a OSS 利用者, I want リポジトリルートに Apache License 2.0 の LICENSE ファイルが配置されている, so that プロジェクトのライセンス条件を正確に把握できる

#### Acceptance Criteria
1. The cupola shall リポジトリルートに `LICENSE` ファイルを配置する
2. The `LICENSE` shall Apache License, Version 2.0 の全文を含む
3. The `LICENSE` shall 著作権表示に正しい年号とプロジェクト名を記載する

### Requirement 2: README.md のライセンスセクション更新
**Objective:** As a OSS 利用者, I want README.md にライセンス情報が記載されている, so that プロジェクトのライセンスをドキュメントから即座に確認できる

#### Acceptance Criteria
1. The `README.md` shall ライセンスセクション（`## License` または同等の見出し）を含む
2. The ライセンスセクション shall Apache License 2.0 であることを明記する
3. The ライセンスセクション shall LICENSE ファイルへの参照を含む

### Requirement 3: Cargo.toml の license フィールド設定
**Objective:** As a Rust パッケージ利用者, I want Cargo.toml に license フィールドが設定されている, so that crates.io やツールチェインでライセンス情報を自動検出できる

#### Acceptance Criteria
1. The `Cargo.toml` shall `license = "Apache-2.0"` フィールドを `[package]` セクションに含む
2. When `cargo metadata` を実行した時, the 出力 shall license フィールドに `Apache-2.0` を表示する
