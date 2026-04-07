# `cupola compress`

完了済み spec を検出し、Claude Code 側の `/cupola:spec-compress` 実行を促すコマンド。

## 役割

このコマンド自身は spec の要約・圧縮を実行しない。実際に行うのは Claude Code の `/cupola:spec-compress` であり、`cupola compress` は対象 spec の有無を確認するだけである。

## オプション

このコマンドにはオプションがない。

## 参照パス

- specs ルートは固定で `.cupola/specs`

## 検出条件

`.cupola/specs/*/spec.json` を走査し、`phase` が次のいずれかなら「完了済み spec」とみなす。

- `implementation-complete`
- `completed`

`archived` など他の phase は対象外。

## 出力

### スキップ時

次のいずれかを出す。

- `specs ディレクトリが存在しません`
- `完了済みの spec が見つかりません`

### 対象あり

完了済み spec 件数を表示し、`/cupola:spec-compress` の実行を案内する。

## 補足

- `spec.json` が存在しないディレクトリは無視される
- `spec.json` の読込や JSON パースに失敗した場合はコマンド自体がエラーになる
