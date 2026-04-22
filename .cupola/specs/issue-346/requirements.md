# 要件定義書

## はじめに

本機能は、Cupola が Claude Code を起動する際に使用している `--dangerously-skip-permissions` フラグを廃止し、Claude Code 標準の Permission 機構 (`.claude/settings.json` の `permissions.allow` / `permissions.deny`) を活用してプロセスの実行範囲を構造的に制限する。

プロジェクト種別ごとの安全なデフォルト権限テンプレートを `cupola init` で配布し、プロンプトインジェクション対策の根本的な強化を実現する。本変更はセキュリティ多層防御戦略 (#320, #321, #319) の中心的施策として位置付けられる。

## 要件

### 要件 1: `--dangerously-skip-permissions` フラグの廃止

**目的:** セキュリティ担当者として、Claude Code プロセスから `--dangerously-skip-permissions` フラグが除去されることで、プロンプトインジェクション発生時の実行範囲を構造的に制限したい。

#### 受け入れ条件

1. When Cupola が Claude Code プロセスを起動するとき、the Cupola shall `--dangerously-skip-permissions` フラグを渡さずにプロセスを起動する。
2. When `cupola init` の steering bootstrap 処理が実行されるとき、the Cupola shall `--dangerously-skip-permissions` フラグなしで Claude Code を呼び出す。
3. The Cupola shall set the Claude Code プロセスの起動ディレクトリを対象リポジトリのルートに設定し、`.claude/settings.json` が自動的に参照されるようにする。

### 要件 2: Claude Code 設定テンプレートの整備

**目的:** 開発者として、プロジェクト種別に合った最小権限のデフォルト設定を `cupola init` で取得し、安全に Claude Code を活用したい。

#### 受け入れ条件

1. The Cupola shall `assets/claude-settings/base.json` を提供し、全テンプレート共通の最小権限 (Read, Write, Edit, 基本 git 操作) と危険コマンドの deny (rm -rf, curl, wget, ssh, git push, gh, WebFetch, WebSearch 等) を含める。
2. The Cupola shall `assets/claude-settings/rust.json`、`typescript.json`、`python.json`、`go.json` を最低限提供し、各スタック固有のビルド・テストコマンドの allow を含める。
3. When テンプレートファイルが参照されるとき、the Cupola shall コンパイル時に組み込まれた (embedded) テンプレートを使用する。
4. The Cupola shall 各テンプレートを `{ "permissions": { "allow": [...], "deny": [...] } }` の JSON 形式で定義する。

### 要件 3: `cupola init --template` オプションの追加

**目的:** 開発者として、`cupola init --template rust` のようにプロジェクト種別を指定し、適切な権限セットをワンコマンドで設定したい。

#### 受け入れ条件

1. When `cupola init` がオプションなしで実行されるとき、the Cupola shall `base.json` テンプレートのみを使用して `.claude/settings.json` を生成する。
2. When `cupola init --template <key>` が実行されるとき、the Cupola shall `base.json` をベースとして `<key>.json` をオーバーレイした設定で `.claude/settings.json` を生成する。
3. When `cupola init --template <key1>,<key2>` のようにカンマ区切りで複数テンプレートが指定されたとき、the Cupola shall 指定された順序に従って各テンプレートをオーバーレイして `.claude/settings.json` を生成する。
4. If 存在しないテンプレートキーが指定されたとき、the Cupola shall エラーメッセージとともに処理を中断し、利用可能なテンプレートキーの一覧を表示する。
5. The Cupola shall `base` キーが `--template` に明示的に指定された場合でも二重適用されないよう処理する。

### 要件 4: 既存 `.claude/settings.json` とのディープマージ

**目的:** 開発者として、既存のカスタマイズ設定を失わずに Cupola 管理の権限テンプレートを適用・更新したい。

#### 受け入れ条件

1. When `cupola init` 実行時に対象リポジトリに既に `.claude/settings.json` が存在するとき、the Cupola shall `permissions.allow` と `permissions.deny` の各配列について、既存値と新規値を union (重複排除) でマージする。
2. When マージ時に同名のスカラーキーが競合するとき、the Cupola shall 既存の値を優先してユーザーのカスタマイズを保持する。
3. When `cupola init --upgrade` が実行されるとき、the Cupola shall 最新のテンプレートを再適用しながら、ユーザーが追加した `allow`/`deny` エントリを保持する。
4. When `.claude/settings.json` が存在しないとき、the Cupola shall マージなしで新規ファイルを生成する。

### 要件 5: Permission 不足時のエラーハンドリング

**目的:** 開発者として、Claude Code が権限不足で処理を停止した際に原因と対処方法を素早く把握したい。

#### 受け入れ条件

1. When Claude Code プロセスが permission denied エラーを含む応答を返したとき、the Cupola shall その応答をセッション失敗として扱い、失敗状態に遷移させる。
2. When permission denied エラーが発生したとき、the Cupola shall 拒否されたツール名または操作の情報をエラーログに出力し、`.claude/settings.json` の `permissions.allow` への追加方法をヒントとして併記する。
3. The Cupola shall `--output-format json` での実行時に、permission denied が発生してもインタラクティブプロンプトに移行しない動作を期待する。

### 要件 6: ドキュメント整備事項

**目的:** コントリビューターおよびユーザーとして、permission 機構の変更内容とテンプレート追加手順を将来的に公式ドキュメントで確認できるようにしたい。

#### 補足事項

1. `SECURITY.md` の Prompt Injection Risk セクションには、`--dangerously-skip-permissions` 廃止と permission 機構の採用、および `cupola init --template` の使い方を追って記載することが望ましい。
2. `CONTRIBUTING.md` には、`assets/claude-settings/<key>.json` を追加することでテンプレートをコントリビュートできる手順と命名規則を追って記載することが望ましい。
3. `SECURITY.md` には、`permissions.allow` を緩めることが攻撃面の拡大につながることを追って明記することが望ましい。
