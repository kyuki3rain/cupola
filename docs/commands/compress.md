# `cupola compress`

完了済み spec を検出し、Claude Code を起動して `/cupola:spec-compress` を実行するコマンド。

## 役割

1. `.cupola/specs/*/spec.json` を走査して完了済み spec の件数を確認する
2. 対象があれば Claude Code を起動し、`/cupola:spec-compress` の実行を指示するプロンプトを渡す

実際の要約・アーカイブ処理は Claude Code の `/cupola:spec-compress` skill が行う。

## 参照パス

specs ルートは `.cupola/specs`。

## 検出条件

`.cupola/specs/*/spec.json` を走査し、`phase` が次のいずれかなら「完了済み spec」とみなす。

- `implementation-complete`
- `completed`

`archived` など他の phase は対象外。

## 実行フロー

### 対象なし

次のいずれかを出して終了する。

- `specs ディレクトリが存在しません`
- `完了済みの spec が見つかりません`

### 対象あり

完了済み spec 件数を表示したうえで、次のコマンドを実行する。

```bash
claude --dangerously-skip-permissions -p \
  "Please run the /cupola:spec-compress slash command to summarize and archive the completed specs."
```

- 成功時は追加メッセージなし
- `claude` 未導入なら `skipped (claude not installed)` 扱い
- その他失敗も `skipped (...)` としてレポートし、`compress` 自体は継続する

## 補足

- `spec.json` が存在しないディレクトリは無視される
- `spec.json` の読込や JSON パースに失敗した場合はコマンド自体がエラーになる
