# Implementation Plan

- [ ] 1. spec-init スキルファイルの実装
- [ ] 1.1 スキルのフロントマターと基本構造を定義する
  - `.claude/commands/cupola/spec-init.md` を新規作成する
  - `description`, `allowed-tools`, `argument-hint` を既存スキルのパターンに従って設定する
  - `allowed-tools` に `Bash, Read, Write, Glob` を含める
  - _Requirements: 1.1, 1.3_

- [ ] 1.2 前提条件チェックと引数バリデーションを実装する
  - spec-id が未指定の場合に使用方法を表示して停止する処理を記述する
  - spec-id に英数字・ハイフン・アンダースコア以外が含まれる場合にエラーを報告する処理を記述する
  - `.cupola/` ディレクトリが存在しない場合に `cupola init` を促すエラーを報告する処理を記述する
  - spec directory が既に存在する場合にスキップして報告する処理を記述する
  - _Requirements: 1.2, 4.1, 4.2, 4.3_

- [ ] 1.3 言語設定の解決ロジックを実装する
  - 引数 `$2` が指定された場合はそれを言語として使用する処理を記述する
  - `$2` が未指定の場合、`.cupola/cupola.toml` から `language` 設定を読み込む処理を記述する
  - `cupola.toml` に言語設定がない場合は `ja` をデフォルト値として使用する処理を記述する
  - _Requirements: 1.3, 1.4_

- [ ] 1.4 spec.json の生成ロジックを実装する
  - `.cupola/settings/templates/specs/init.json` テンプレートを読み込む処理を記述する
  - `{{FEATURE_NAME}}`, `{{TIMESTAMP}}`, `{{LANGUAGE}}` プレースホルダーを実際の値で置換する処理を記述する
  - テンプレートが存在しない場合のフォールバック JSON（インライン値）を記述する
  - `.cupola/specs/{spec-id}/spec.json` として書き込む処理を記述する
  - _Requirements: 2.2, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 1.5 requirements.md の生成と完了メッセージを実装する
  - `.cupola/settings/templates/specs/requirements-init.md` テンプレートを読み込む処理を記述する
  - テンプレートが存在しない場合のフォールバックコンテンツを記述する
  - `.cupola/specs/{spec-id}/requirements.md` として書き込む処理を記述する
  - 生成したファイルのパスを表示し、次のステップ（`/cupola:spec-design {spec-id}`）への案内を表示する
  - _Requirements: 2.1, 2.3, 2.5, 3.7, 1.5_

- [ ] 2. Rust デーモンへのスキル登録
- [ ] 2.1 init_file_generator.rs に spec-init.md を登録する
  - `CLAUDE_CODE_ASSETS` 配列に `(".claude/commands/cupola/spec-init.md", include_str!(...))` エントリを追加する
  - `include_str!()` のパスは他のスキルエントリと同一パターンに従う
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 2.2 ビルドと既存テストの通過を確認する (P)
  - `devbox run build` でコンパイルエラーがないことを確認する
  - `devbox run test` で既存テストがすべて通過することを確認する
  - `devbox run clippy` で警告がないことを確認する
  - _Requirements: 5.3_
