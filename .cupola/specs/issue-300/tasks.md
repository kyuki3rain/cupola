# Implementation Plan

- [x] 1. spec-init スキルファイルの実装
- [x] 1.1 spec-init スキルを追加し、必要なメタデータを設定する
  - 既存スキル群と一貫したフォーマットでスキルを定義する
  - スキルの説明・引数ヒントを他スキルと統一された形式で記述する
  - ファイル操作に必要なツールアクセスを許可する
  - _Requirements: 1.1, 1.3_

- [x] 1.2 前提条件チェックと引数バリデーションを実装する
  - spec-id が未指定の場合に使用方法を表示して停止する処理を記述する
  - spec-id に英数字・ハイフン・アンダースコア以外が含まれる場合にエラーを報告する処理を記述する
  - `.cupola/` ディレクトリが存在しない場合に `cupola init` を促すエラーを報告する処理を記述する
  - spec directory が既に存在する場合にスキップして報告する処理を記述する
  - _Requirements: 1.2, 4.1, 4.2, 4.3_

- [x] 1.3 言語設定の解決ロジックを実装する
  - 引数 `$2` が指定された場合はそれを言語として使用する処理を記述する
  - `$2` が未指定の場合、`.cupola/cupola.toml` から `language` 設定を読み込む処理を記述する
  - `cupola.toml` に言語設定がない場合は `ja` をデフォルト値として使用する処理を記述する
  - _Requirements: 1.3, 1.4_

- [x] 1.4 spec.json の生成ロジックを実装する
  - `.cupola/settings/templates/specs/init.json` テンプレートを読み込む処理を記述する
  - `{{FEATURE_NAME}}`, `{{TIMESTAMP}}`, `{{LANGUAGE}}` プレースホルダーを実際の値で置換する処理を記述する
  - テンプレートが存在しない場合のフォールバック JSON（インライン値）を記述する
  - `.cupola/specs/{spec-id}/spec.json` として書き込む処理を記述する
  - _Requirements: 2.2, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [x] 1.5 requirements.md の生成と完了メッセージを実装する
  - `.cupola/settings/templates/specs/requirements-init.md` テンプレートを読み込む処理を記述する
  - テンプレートが存在しない場合のフォールバックコンテンツを記述する
  - `.cupola/specs/{spec-id}/requirements.md` として書き込む処理を記述する
  - 生成したファイルのパスを表示し、次のステップ（`/cupola:spec-design {spec-id}`）への案内を表示する
  - _Requirements: 2.1, 2.3, 2.5, 3.7, 1.5_

- [x] 2. Rust デーモンへのスキル登録
- [x] 2.1 cupola init 後に spec-init スキルが利用可能になるよう登録する
  - 他の既存スキルと同様のパターンで新スキルをインストール対象に追加する
  - 既存のインストールロジックは変更せず、エントリ追加のみで対応する
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 2.2 ビルドと既存テストの通過を確認する (P)
  - `devbox run build` でコンパイルエラーがないことを確認する
  - `devbox run test` で既存テストがすべて通過することを確認する
  - `devbox run clippy` で警告がないことを確認する
  - _Requirements: 5.3_
