# doc-consistency-fix サマリー

## Feature
v0.1プレリリース監査で発見された4件のドキュメント・設定ファイル不整合の一括修正。

## 要件サマリ
- `.cupola/steering/tech.md` のCLIサブコマンド記述を `run/init/status` から `start/stop/init/status/doctor` に更新。
- `CHANGELOG.md` の Added セクションに `cupola start --daemon` と `cupola stop` を追記。
- `README.md` / `README.ja.md` に Unix (macOS/Linux) only の制約を明記。
- `.gitignore` に `.env` を追加しトークン等の誤コミットを防止。

## アーキテクチャ決定
- ドキュメント・設定のみの変更でコードには手を入れない方針。
- 4件の独立した軽微変更は単一 PR で一括対応しレビューコストを最小化。
- Unix only 制約は `nix` クレート (`cfg(unix)`) 依存に起因する旨もコメントで添える。

## コンポーネント
- `.cupola/steering/tech.md`、`CHANGELOG.md`、`README.md` / `README.ja.md`、`.gitignore`

## 主要インターフェース
なし（ドキュメント・設定のみ）。

## 学び/トレードオフ
- `.env` パターンは `.env.example` を巻き込まないよう注意。
- CHANGELOG のバージョン未確定問題は `[Unreleased]` または `v0.1.0` セクションへの追記で吸収。
- ステアリングが実装から乖離しやすいことを示し、リリース前チェック項目化の必要性を示唆。
