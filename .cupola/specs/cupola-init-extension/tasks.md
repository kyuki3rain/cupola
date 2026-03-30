# Implementation Plan

- [ ] 1. InitFileGenerator の実装
- [ ] 1.1 (P) cupola.toml 雛形生成機能を実装する
  - cupola.toml が存在しない場合に、必須フィールド（owner, repo, default_branch）を空欄、オプションフィールドをコメントアウトした雛形を生成する
  - cupola.toml が既に存在する場合はスキップし、ログを出力する
  - 戻り値で実際に生成したかスキップしたかを判別できるようにする
  - _Requirements: 2.1, 2.2_

- [ ] 1.2 (P) steering テンプレートコピー機能を実装する
  - テンプレートディレクトリ（.cupola/settings/templates/steering/）の存在を確認し、不在時はスキップしてログ出力する
  - steering ディレクトリが空の場合のみテンプレートファイルをコピーする
  - steering ディレクトリにファイルが既に存在する場合はスキップする
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 1.3 (P) .gitignore エントリ追記機能を実装する
  - マーカーコメント「# cupola」の有無で重複を検出する
  - マーカーが未追加の場合、cupola 用エントリ（DB ファイル、WAL 関連、ログ、worktrees、inputs）を追記する
  - .gitignore ファイルが存在しない場合は新規作成する
  - 既にマーカーが存在する場合はスキップする
  - _Requirements: 4.1, 4.2, 4.3_

- [ ] 2. InitUseCase の実装と bootstrap 統合
- [ ] 2.1 InitUseCase を作成し、全初期化ステップを統括する
  - ディレクトリ作成、SQLite 初期化、cupola.toml 生成、steering コピー、.gitignore 追記の5ステップを順次実行する
  - 各ステップの結果を InitReport に集約して返却する
  - 対話的入力・cc-sdd インストール・GitHub ラベル作成は行わない
  - _Requirements: 1.1, 1.2, 5.1, 5.2, 6.1, 6.2, 6.3_

- [ ] 2.2 app.rs の Command::Init ハンドラを InitUseCase 呼び出しに置き換える
  - 既存のインライン SQLite 初期化コードを削除し、InitUseCase::run() に委譲する
  - InitReport の内容に基づいて完了メッセージを出力する
  - _Requirements: 1.1, 1.2_

- [ ] 3. テストの実装
- [ ] 3.1 (P) InitFileGenerator の各メソッドのユニットテストを追加する
  - cupola.toml: 新規生成と既存スキップの2パターン
  - steering: コピー成功、既存スキップ、テンプレート不在スキップの3パターン
  - .gitignore: 新規追記、重複スキップ、ファイル新規作成の3パターン
  - 一時ディレクトリを使用してファイルシステム操作を検証する
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3_

- [ ] 3.2 (P) InitUseCase の統合テストを追加する
  - 空ディレクトリでの初回実行で全ファイルが生成されることを検証する
  - 既にファイルが存在する状態で実行し、全ステップがスキップされることを検証する
  - 2回連続実行で2回目が全スキップかつファイル内容不変であることを検証する
  - テンプレートディレクトリ不在時に steering がスキップされ、他は正常動作することを検証する
  - _Requirements: 5.1, 5.2_
