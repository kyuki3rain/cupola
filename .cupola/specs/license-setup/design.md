# Design Document

## Overview
**Purpose**: Apache License 2.0 の LICENSE ファイルを作成し、関連ファイル（README.md、Cargo.toml）を更新することで、OSS としてのライセンス明示を完了する。

**Users**: OSS 利用者、Rust パッケージ利用者がライセンス条件を把握するために使用する。

**Impact**: リポジトリルートに LICENSE ファイルが追加され、README.md の License セクションと Cargo.toml の package メタデータが更新される。

### Goals
- Apache License 2.0 の全文を含む LICENSE ファイルをリポジトリルートに配置する
- README.md のライセンスセクションを実際のライセンス情報で更新する
- Cargo.toml に SPDX 識別子による license フィールドを設定する

### Non-Goals
- デュアルライセンス対応（スタンドアロンツールのため不要）
- ライセンスヘッダーの各ソースファイルへの追加
- ライセンスチェックの CI パイプラインへの組み込み

## Requirements Traceability

| Requirement | Summary | Components | Interfaces | Flows |
|-------------|---------|------------|------------|-------|
| 1.1 | LICENSE ファイルをリポジトリルートに配置 | LicenseFile | — | — |
| 1.2 | Apache License 2.0 の全文を含む | LicenseFile | — | — |
| 1.3 | 著作権表示に正しい年号とプロジェクト名を記載 | LicenseFile | — | — |
| 2.1 | README.md にライセンスセクションを含む | ReadmeUpdate | — | — |
| 2.2 | Apache License 2.0 であることを明記 | ReadmeUpdate | — | — |
| 2.3 | LICENSE ファイルへの参照を含む | ReadmeUpdate | — | — |
| 3.1 | Cargo.toml に license = "Apache-2.0" を設定 | CargoTomlUpdate | — | — |
| 3.2 | cargo metadata で Apache-2.0 が表示される | CargoTomlUpdate | — | — |

## Components and Interfaces

本フィーチャーはコード変更を伴わず、静的ファイルの追加・編集のみで構成される。アーキテクチャ図・サービスインターフェースは不要。

| Component | Domain/Layer | Intent | Req Coverage | Key Dependencies | Contracts |
|-----------|-------------|--------|--------------|-----------------|-----------|
| LicenseFile | リポジトリルート | Apache License 2.0 全文の配置 | 1.1, 1.2, 1.3 | なし | — |
| ReadmeUpdate | リポジトリルート | README.md のライセンスセクション更新 | 2.1, 2.2, 2.3 | LicenseFile (P2) | — |
| CargoTomlUpdate | リポジトリルート | Cargo.toml の license フィールド設定 | 3.1, 3.2 | なし | — |

### リポジトリルート

#### LicenseFile

| Field | Detail |
|-------|--------|
| Intent | Apache License 2.0 の全文を含む LICENSE ファイルの作成 |
| Requirements | 1.1, 1.2, 1.3 |

**Responsibilities & Constraints**
- リポジトリルートに `LICENSE` というファイル名で配置する（拡張子なし）
- Apache License, Version 2.0 の標準テキスト全文を含む
- 著作権行: `Copyright 2026 cupola contributors`

**Dependencies**
- なし

**Implementation Notes**
- Apache Software Foundation の公式テキストをそのまま使用する
- 著作権年は 2026、著作権者は `cupola contributors` とする

#### ReadmeUpdate

| Field | Detail |
|-------|--------|
| Intent | README.md の既存 License セクションをライセンス情報で更新 |
| Requirements | 2.1, 2.2, 2.3 |

**Responsibilities & Constraints**
- 既存の `## License` セクションの内容を置換する（行番号ではなく見出しを基準に特定する）
- Apache License 2.0 であることを明記する
- LICENSE ファイルへの参照リンクを含める

**Dependencies**
- Inbound: なし
- Outbound: LicenseFile — LICENSE ファイルの存在を前提とする (P2)

**Implementation Notes**
- 既存の TBD テキストを実際のライセンス情報に置換する
- Table of Contents の `[License](#license)` は既に存在するため変更不要

#### CargoTomlUpdate

| Field | Detail |
|-------|--------|
| Intent | Cargo.toml の [package] セクションに license フィールドを追加 |
| Requirements | 3.1, 3.2 |

**Responsibilities & Constraints**
- `[package]` セクションに `license = "Apache-2.0"` を追加する
- SPDX 2.1 ライセンス式に準拠した識別子を使用する

**Dependencies**
- なし

**Implementation Notes**
- `edition` フィールドの直後に `license` フィールドを配置する（Cargo.toml の慣例的な順序）
- `license-file` フィールドは不要（Apache-2.0 は標準ライセンスとして認識される）

## Testing Strategy

### 検証項目
- **LICENSE ファイル**: `LICENSE` ファイルがリポジトリルートに存在し、Apache License 2.0 の全文を含むことを目視確認
- **README.md**: `## License` セクションに Apache License 2.0 の記載と LICENSE ファイルへの参照があることを目視確認
- **Cargo.toml**: `cargo metadata --format-version=1 | jq '.packages[0].license'` の出力が `"Apache-2.0"` であることを確認
