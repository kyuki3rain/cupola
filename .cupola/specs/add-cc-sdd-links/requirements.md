# Requirements Document

## Project Description (Input)
プロジェクト内の各ドキュメントファイル（README.md, README.ja.md, CHANGELOG.md, ISSUE_TEMPLATE, steering/product.md）に記載されている cc-sdd（外部ツール）への言及箇所に、GitHub リポジトリ（https://github.com/gotalab/cc-sdd）へのリンクを追加する

## はじめに

Cupola プロジェクトの各ドキュメントでは cc-sdd（仕様駆動開発ツール）を外部ツールとして参照しているが、現状はテキストのみで記載されており、利用者がツールの詳細を確認するための外部リンクが提供されていない。本フィーチャーでは、該当する 5 ファイルの初出箇所に cc-sdd GitHub リポジトリへのリンクを追加することで、ドキュメントの利便性と情報の参照可能性を向上させる。

## Requirements

### Requirement 1: Markdown ドキュメントへの cc-sdd リンク追加

**Objective:** ドキュメント利用者として、cc-sdd への参照箇所でリンクが提供されていることを望む。それにより、外部ツールの詳細を追加の検索なしに即座に確認できるようにするため。

#### Acceptance Criteria

1. The プロジェクトドキュメント shall `README.md` の cc-sdd 言及箇所（初出）を `[cc-sdd](https://github.com/gotalab/cc-sdd)` 形式のリンクとして表示する
2. The プロジェクトドキュメント shall `README.ja.md` の cc-sdd 言及箇所（初出）を `[cc-sdd](https://github.com/gotalab/cc-sdd)` 形式のリンクとして表示する
3. The プロジェクトドキュメント shall `CHANGELOG.md` の cc-sdd 言及箇所（初出）を `[cc-sdd](https://github.com/gotalab/cc-sdd)` 形式のリンクとして表示する
4. The プロジェクトドキュメント shall `.cupola/steering/product.md` の cc-sdd 言及箇所（初出）を `[cc-sdd](https://github.com/gotalab/cc-sdd)` 形式のリンクとして表示する

### Requirement 2: YAML ファイルへの cc-sdd URL 追記

**Objective:** ドキュメント利用者として、YAML 形式のテンプレートファイルでも cc-sdd の URL を参照できることを望む。それにより、Issue テンプレートを確認する際にも外部ツールへのアクセスができるようにするため。

#### Acceptance Criteria

1. The プロジェクトドキュメント shall `.github/ISSUE_TEMPLATE/cupola-task.yml` の cc-sdd 言及箇所に URL `https://github.com/gotalab/cc-sdd` を括弧付きで併記する
2. If YAML ファイルの構文上マークダウンリンク記法が使用できない場合, the プロジェクトドキュメント shall URL をプレーンテキストとして括弧付きで記載する（例: `cc-sdd (https://github.com/gotalab/cc-sdd)`）

### Requirement 3: リンクの正確性と既存フォーマットの維持

**Objective:** ドキュメント管理者として、追加されたリンクが正確であり、既存のフォーマットや文体が維持されていることを望む。それにより、ドキュメントの品質と一貫性を損なわずにリンクを追加できるようにするため。

#### Acceptance Criteria

1. The プロジェクトドキュメント shall cc-sdd リンクが `https://github.com/gotalab/cc-sdd` を正確に指すことを保証する
2. The プロジェクトドキュメント shall 各ファイルの既存のインデントス、行構成、文体を維持した状態でリンクを追加する
3. When リンクが初出箇所のみに追加された場合, the プロジェクトドキュメント shall 同一ファイル内の後続の cc-sdd 言及はリンクなしのテキストのままとする
4. The プロジェクトドキュメント shall ソースコード内ファイル（`prompt.rs`、`init_file_generator.rs` 等）にはリンクを追加しない（内部指示・ログのため対象外）
