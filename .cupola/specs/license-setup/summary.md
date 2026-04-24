# license-setup サマリ

## Feature
Apache License 2.0 を OSS プロジェクト cupola に正式適用するため、`LICENSE` ファイルの配置、README.md ライセンスセクションの更新、`Cargo.toml` の `license` フィールド設定を行った。

## 要件サマリ
- リポジトリルートに Apache License 2.0 全文の `LICENSE` ファイルを配置。
- 著作権行は `Copyright 2026 cupola contributors`。
- README.md の既存 `## License` セクション（TBD）を Apache-2.0 明記 + LICENSE ファイルリンクに更新。
- `Cargo.toml` `[package]` に `license = "Apache-2.0"` を追加し、`cargo metadata` で検出可能に。

## アーキテクチャ決定
- **ライセンスは Apache-2.0 単独**: デュアルライセンス (MIT/Apache-2.0) はスタンドアロンツールでは不要と判断。
- **著作権者は `cupola contributors`** (採用): 個人名 `kyuki3rain` の代替案もあったが、OSS の慣例に合わせコミュニティを代表する表記を選択。
- **SPDX 識別子利用 + `license-file` は設定しない**: Apache-2.0 は標準ライセンスとして crates.io ツール側で認識されるため `license-file` は冗長。
- **ソースファイルへのライセンスヘッダー追加、ライセンスチェック CI は非スコープ**。

## コンポーネント
- `LICENSE`（新規）: Apache Software Foundation 公式テキスト全文。
- `README.md`: 既存 `## License` セクションの文面置換のみ。
- `Cargo.toml`: `[package]` の `edition` 直後に `license = "Apache-2.0"` 追記。

## 主要インターフェース
コード変更なし。検証は `cargo metadata --format-version=1 | jq '.packages[0].license'` が `"Apache-2.0"` を返すことで行う。

## 学び / トレードオフ
- 静的ファイルの追加・編集のみでランタイムへの影響ゼロ、リスクは極小。
- 個人著作権主張は弱まるが OSS 慣例に沿った表記を優先。
- README.md に既に `## License` セクションと TOC リンクが存在していたため、新規セクション追加不要で差分が最小に収まった。
