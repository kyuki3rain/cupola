# translate-steering-to-english サマリ

## Feature
`.cupola/steering/` 配下の 3 ファイル (`product.md` / `tech.md` / `structure.md`) を日本語から英語に翻訳。OSS として公開済みで README も英語化されているため、AI エージェント (Claude Code) が参照する技術コンテキストの言語的一貫性を確保する。spec ドキュメント (`.cupola/specs/`) はレビュアー向けに日本語を維持し対象外。

## 要件サマリ
- **product.md**: Core Capabilities / Target Use Cases / Value Proposition 全セクションを英語化。情報量維持、OSS として自然な英語表現。
- **tech.md**: Architecture / Core Technologies / Key Libraries / Development Standards / Development Environment / Key Technical Decisions 全セクションを英語化。Key Libraries テーブル構造維持、コードブロック内の日本語コメント (`# ビルド` 等) も英訳、クレート名・コマンド・型名は原文維持。
- **structure.md**: Organization Philosophy / Directory Patterns / Naming Conventions / Code Organization Principles 全セクションを英語化。レイヤー名（domain/application/adapter/bootstrap）、ディレクトリパス、型名・関数名・定数名は変更しない。命名規約記法（snake_case 等）は原文維持。
- **共通品質**: Markdown フォーマット（見出し、リスト、テーブル、コードブロック）完全保持、固有名詞（Cupola, Claude Code, cc-sdd）維持、意訳優先で原文ニュアンスを保持、spec への影響ゼロ。

## アーキテクチャ決定
- **ファイル単位の逐次翻訳** (採用): 各ファイルの性質が異なる（製品概要 vs 技術仕様 vs 構造説明）ため、個別に品質確認しやすい。一括翻訳案は品質検証が難しく不採用。
- **翻訳対象と非翻訳対象の明確な境界定義**: 固有名詞（Cupola, Claude Code, cc-sdd, GitHub, Issue, PR）、クレート名（octocrab, rusqlite 等）、コマンド（cargo build 等）、型名（GitHubClientImpl, PollingUseCase 等）、関数名・定数名、ディレクトリパス（`src/domain/` 等）、命名規約記法（snake_case, UpperCamelCase, SCREAMING_SNAKE_CASE）はすべて原文維持。説明文・セクション見出し・テーブルヘッダー/セル内容・コードブロック内コメントのみ翻訳。
- **意訳優先**: 直訳ではなく原文のニュアンスを優先し技術的正確性を担保。日本語特有の表現（「ワンストップ自動化」等）は自然な英語に意訳。
- **spec ドキュメントは対象外**: レビュアー（日本語話者）の可読性を優先し、`.cupola/specs/` は日本語のまま維持する二言語方針を明確化。
- **CLAUDE.md や他のプロジェクトドキュメントの翻訳は非スコープ**。

## コンポーネント
- `.cupola/steering/product.md`: 4 セクション構成、概要 + Core Capabilities（4 機能）+ Target Use Cases（3 ケース）+ Value Proposition を全文英訳。
- `.cupola/steering/tech.md`: 6 セクション + Key Libraries テーブル（ヘッダー 用途/クレート/パターン → Purpose/Crate/Pattern）+ コードブロックを全文英訳。技術用語の誤訳リスク最大のため最も慎重な翻訳が必要。
- `.cupola/steering/structure.md`: 4 セクション構成、Directory Patterns の 4 レイヤー（Domain / Application / Adapter / Bootstrap）の Purpose / Contains / Rule 説明、Naming Conventions、Code Organization Principles を英訳。コード識別子の維持に最大の注意。
- コード変更・ビルドへの影響なし。

## 主要インターフェース
ドキュメント翻訳のみのためコード API なし。検証は目視 + セクション数/箇条書き項目数の一致確認 + Markdown フォーマット保持確認で実施。

## 学び / トレードオフ
- steering (英語) と spec (日本語) の言語を分離する二言語運用は、AI 参照コンテキスト（英語が効率的）とレビュアーの可読性（日本語が直観的）のバランスを取った現実解。将来的に spec も英語化する場合は別 Issue で対応。
- `tech.md` のテーブルは日本語→英語でカラム幅が変動するため、Markdown 表のアライメント維持がややトリッキー。
- コードブロック内の `# ビルド` のような日本語コメントを英訳する決定は、AI エージェントが引用した際の一貫性を重視。クレート名やコマンドは原文維持と明確に区別。
- 命名規約記法（`snake_case`、`UpperCamelCase` など）を翻訳対象外とする判断は、技術ドキュメントの標準語彙として世界共通のため当然だが、仕様として明文化したことで翻訳時の判断コストを抑制。
