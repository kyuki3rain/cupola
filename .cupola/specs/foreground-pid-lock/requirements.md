# 要件ドキュメント

## プロジェクト説明 (入力)
foreground モードの二重起動防止 (PID ファイル導入): foreground モードに PID ファイルを用いた二重起動防止を追加する。daemon と foreground で同じチェックロジックを共有し、foreground 同士、foreground と daemon の全組み合わせで二重起動を防止する。

## イントロダクション

Cupola は GitHub Issues を自動処理するローカル常駐エージェントである。現在、`daemon` モードでは PID ファイルによる二重起動防止が実装済みだが、`foreground` モードにはそのチェックがない。その結果、daemon + foreground の組み合わせ、または foreground 同士で同一の SQLite DB へ複数プロセスが同時アクセスし、Issue の二重処理が発生するリスクがある。本フィーチャーは `foreground` モードにも同じ PID ファイルチェックロジックを導入することで、全起動モードの組み合わせにおける二重起動を防止する。

## 要件

### 要件 1: foreground モードの起動時 PID ファイルチェック

**目的:** オペレーターとして、foreground モードで Cupola を起動したとき、既に同一 PID ファイルを持つプロセス (daemon または foreground) が動作中であればエラーで即座に終了してほしい。そうすることで、Issue の二重処理による DB 競合を防止できる。

#### 受け入れ基準
1. When foreground モードで `cupola start` が実行され、PID ファイルが存在し記載 PID のプロセスが生存中である, the Cupola shall 起動を中断し `"cupola is already running (pid=<PID>)"` を含むエラーを返す。
2. When foreground モードで `cupola start` が実行され、PID ファイルが存在するが記載 PID のプロセスが存在しない（ゾンビ PID ファイル）, the Cupola shall PID ファイルを削除してから起動処理を継続する。
3. When foreground モードで `cupola start` が実行され、PID ファイルが存在しない, the Cupola shall エラーなく起動処理を継続する。
4. If PID ファイルの読み込みに IO エラーが発生した, the Cupola shall 起動を中断し当該エラーを返す。

### 要件 2: foreground モード実行中の PID ファイル書き込み

**目的:** オペレーターとして、foreground モードで Cupola が起動したとき、自身の PID が PID ファイルに記録されてほしい。そうすることで、後続の起動試行（daemon / foreground 問わず）が二重起動をチェックできるようになる。

#### 受け入れ基準
1. When PID チェックを通過して foreground モードのポーリング開始前, the Cupola shall 自プロセスの PID を `cupola.pid` ファイルに書き込む。
2. If PID ファイルへの書き込みに失敗した, the Cupola shall 起動を中断しエラーを返す。
3. The Cupola shall foreground モードの PID ファイルパスとして daemon モードと同一のパス (`<config_dir>/cupola.pid`) を使用する。

### 要件 3: foreground モード終了時の PID ファイル削除

**目的:** オペレーターとして、foreground モードで Cupola が終了したとき（正常・エラー問わず）、PID ファイルが自動的に削除されてほしい。そうすることで、次回起動時にゾンビ PID ファイルが残らない。

#### 受け入れ基準
1. When foreground モードが正常終了した, the Cupola shall `cupola.pid` ファイルを削除する。
2. When foreground モードがエラーにより終了した, the Cupola shall `cupola.pid` ファイルを削除してから元のエラーを返す。
3. If PID ファイルの削除に失敗した, the Cupola shall 削除エラーを握り潰し、元の終了結果（Ok または元の Err）をそのまま返す。

### 要件 4: daemon モードと foreground モードの相互排他

**目的:** オペレーターとして、daemon と foreground がどの順番で起動されても二重起動防止が機能してほしい。そうすることで、いかなる起動順序でも Issue の二重処理が発生しない。

#### 受け入れ基準
1. When daemon が動作中に foreground モードで `cupola start` を実行した, the Cupola shall 起動を拒否しエラーを返す。
2. When foreground が動作中に daemon モードで `cupola start` を実行した, the Cupola shall 起動を拒否しエラーを返す（daemon の既存チェックロジックが foreground の PID ファイルも検出できること）。
3. While 同一ホスト上で Cupola プロセスが1つ動作中（foreground または daemon）, the Cupola shall 同一 `cupola.pid` ファイルを共有することで相互排他を保証する。
