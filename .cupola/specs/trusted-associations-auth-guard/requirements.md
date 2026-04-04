# 要件ドキュメント

## はじめに

Cupola は GitHub Issues や PR レビューコメントなど、外部ユーザーが書き込める入力を Claude Code に渡して実行する。公開リポジトリでは、悪意あるプロンプトインジェクション攻撃のリスクがある。本機能は GitHub API が返す `author_association` フィールドを用いて入力元を認証し、信頼できるユーザーからの入力のみを処理することで、プロンプトインジェクション攻撃を防ぐ。

## 要件

### 要件 1: trusted_associations 設定フィールド

**目的:** リポジトリ管理者として、信頼するユーザーの association レベルを設定ファイルで制御したい。そうすることで、リポジトリのセキュリティポリシーに応じた柔軟なアクセス制御が可能になる。

#### 受け入れ基準

1. The Cupola system shall `trusted_associations` フィールドを `cupola.toml` の設定として読み込むこと。
2. When `trusted_associations` が設定ファイルに存在しない場合、the Cupola system shall デフォルト値として `["OWNER", "MEMBER", "COLLABORATOR"]` を使用すること。
3. Where `trusted_associations = ["all"]` が設定されている場合、the Cupola system shall association チェックをスキップすること。
4. The Cupola system shall `OWNER`、`MEMBER`、`COLLABORATOR`、`CONTRIBUTOR`、`FIRST_TIMER`、`FIRST_TIME_CONTRIBUTOR`、`NONE` の各値を有効な association として認識すること。
5. If `trusted_associations` に無効な値が含まれる場合、the Cupola system shall 起動時にエラーをログ出力し、処理を停止すること。

---

### 要件 2: agent:ready ラベル付与者の association チェック

**目的:** リポジトリ管理者として、`agent:ready` ラベルを信頼できるユーザーのみが有効に付与できるようにしたい。そうすることで、外部の悪意あるユーザーが Cupola のエージェント実行をトリガーするリスクを排除できる。

#### 受け入れ基準

1. When `agent:ready` ラベルが Issue に付与されたイベントを検出した場合、the Cupola system shall GitHub Timeline API を使用してラベルを付与した actor の `login` を取得すること。
2. When 取得した actor の `login` に基づき、別 API（例: collaborators permission や organization membership）を使用して判定した association が `trusted_associations` リストに含まれる場合、the Cupola system shall 通常のワークフローを続行すること。
3. When 取得した actor の `login` に基づき、別 API（例: collaborators permission や organization membership）を使用して判定した association が `trusted_associations` リストに含まれない場合、the Cupola system shall `agent:ready` ラベルを Issue から削除すること。
4. When `agent:ready` ラベルを削除した場合、the Cupola system shall 該当 Issue にコメントを投稿し、ラベルを削除した理由と信頼できる association レベルを通知すること。
5. If GitHub Timeline API または association 判定に必要な別 API の呼び出しが失敗した場合、the Cupola system shall エラーをログ出力し、安全側の原則としてワークフローを続行しないこと。
6. The Cupola system shall association チェックの結果（許可/拒否）をログに記録すること。

---

### 要件 3: PR レビューコメントの author_association フィルタリング

**目的:** リポジトリ管理者として、信頼できるユーザーからのレビューコメントのみが Claude Code への入力として渡されるようにしたい。そうすることで、外部コントリビューターによるプロンプトインジェクション攻撃を防止できる。

#### 受け入れ基準

1. When PR レビュースレッドを `review_threads.json` に書き出す場合、the Cupola system shall 各コメントの `author_association` を確認すること。
2. When コメントの author_association が `trusted_associations` リストに含まれる場合、the Cupola system shall そのコメントを `review_threads.json` に含めること。
3. When コメントの author_association が `trusted_associations` リストに含まれない場合、the Cupola system shall そのコメントを `review_threads.json` から除外すること。
4. The Cupola system shall 除外されたコメントの数と author の association レベルをログに記録すること。
5. Where `trusted_associations = ["all"]` が設定されている場合、the Cupola system shall すべてのレビューコメントを `review_threads.json` に含めること。

---

### 要件 4: セキュリティドキュメントの整備

**目的:** ユーザーおよびコントリビューターとして、プロンプトインジェクションリスクと trusted_associations 機能について理解できるようにしたい。そうすることで、安全にリポジトリを設定・運用できる。

#### 受け入れ基準

1. The Cupola system shall リポジトリルートに `SECURITY.md` を提供すること。このファイルはプロンプトインジェクションリスクの説明と `trusted_associations` 設定の説明を含むこと。
2. The Cupola system shall README またはドキュメントに Fork ワークフローの案内を含めること。外部コントリビューターが fork 上で Cupola を使い、upstream へ PR を出す方法を説明すること。
3. The Cupola system shall `cupola.toml` の設定例として `trusted_associations` の設定方法をドキュメントに記載すること。
