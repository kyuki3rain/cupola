# 実装タスクリスト: issue-346

- [x] 1. `--dangerously-skip-permissions` フラグの削除

- [x] 1.1 Claude Code プロセス起動時の permission フラグを除去する
  - `--dangerously-skip-permissions` フラグを渡さずに Claude Code が起動されること
  - フラグ除去後もプロセスが正常に起動されることをユニットテストで検証する
  - _Requirements: 1.1_

- [x] 1.2 steering bootstrap 呼び出しからフラグを除去する
  - steering bootstrap フローで Claude Code が `--dangerously-skip-permissions` なしで呼び出されること
  - _Requirements: 1.2_

- [x] 2. Claude Settings ドメインモデルの追加

- [x] 2.1 (P) `ClaudeSettings` 値オブジェクトを domain 層に追加する
  - permission 設定を表す値オブジェクトが domain 層に存在すること
  - `allow` と `deny` の配列を持ち、JSON シリアライズ / デシリアライズが機能すること
  - フィールド欠落時は空配列にフォールバックすること
  - _Requirements: 2.4_

- [x] 3. Permission テンプレートファイルの追加

- [x] 3.1 (P) `base.json` テンプレートを作成する
  - 全プロジェクト共通の最小権限設定テンプレートが提供されること
  - git 操作・ファイル操作の allow と危険なコマンドの deny が含まれること
  - _Requirements: 2.1_

- [x] 3.2 (P) スタック別テンプレートを作成する
  - rust / typescript / python / go の各スタックに対応したテンプレートが提供されること
  - 各スタックのビルド・テストコマンドが allow に含まれること
  - _Requirements: 2.2_

- [x] 4. TemplateManager の実装

- [x] 4.1 テンプレートのコンパイル時埋め込みと TemplateManager 骨格を作成する
  - テンプレートファイルがコンパイル時にバイナリに埋め込まれること
  - 未知テンプレートキーを検出してエラーを返すロジックが機能すること
  - _Requirements: 2.3, 3.4_

- [x] 4.2 テンプレートのロード・マージロジックを実装する
  - 複数テンプレートを指定順にオーバーレイして権限設定を合成できること
  - `base` は常に先頭に 1 回だけ適用され、重複が防止されること
  - 利用可能なテンプレートキーを一覧取得できること
  - ユニットテストで base のみ / 複数テンプレート / 未知キー / base 重複入力 / 空スライスの各ケースを検証する
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.3, 3.5_

- [x] 5. InitFileGenerator への settings.json 生成・マージ機能追加

- [x] 5.1 settings.json 書き込みインターフェースを追加する
  - ファイル生成コンポーネントが `ClaudeSettings` を受け取り settings.json を書き込める契約が定義されること
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 5.2 settings.json 書き込みと deep merge を実装する
  - `.claude/settings.json` が存在しない場合は新規生成されること
  - 既存ファイルがある場合は `allow`/`deny` が union マージされ、スカラーキーは既存値が優先されること
  - ネストオブジェクトは再帰的にマージされること
  - ユニットテストで新規ファイル / 既存 allow への union / スカラー既存優先 / --upgrade の各ケースを検証する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 6. CLI `--template` オプションと InitUseCase の拡張

- [x] 6.1 CLI に `--template` オプションを追加する
  - `cupola init --template <key>` でテンプレートキーを指定できること
  - カンマ区切りで複数テンプレートを同時に指定できること
  - 未指定時は空リストとして扱われ、base のみが適用されること
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 6.2 テンプレート選択・settings.json 書き込みを統合する
  - `cupola init` 実行時にテンプレートが選択され `.claude/settings.json` が生成されること
  - 未知テンプレートキーを指定した場合に利用可能キー一覧を示すエラーが表示されること
  - 統合テストで base のみ / --template rust / 既存 settings.json あり / --upgrade の各シナリオを検証する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 4.4_

- [x] 7. Permission Denied エラーハンドリングの改善

- [x] 7.1 Permission Denied レスポンスの検知とエラーログを実装する
  - Claude Code から permission denied レスポンスを受信した場合にセッション失敗として扱われること
  - permission denied 発生時に拒否されたツール名と allow 追加ヒントがエラーログに出力されること
  - _Requirements: 5.1, 5.2, 5.3_
