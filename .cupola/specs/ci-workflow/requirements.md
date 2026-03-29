# Requirements Document

## Introduction
GitHub Actions を用いた CI ワークフローを `.github/workflows/ci.yml` として追加する。PR（main ブランチ向け）および push（main ブランチ）をトリガーとして、フォーマットチェック・静的解析・ユニットテストを自動実行し、コード品質を継続的に担保する。

## Requirements

### Requirement 1: CI トリガー設定
**Objective:** As a 開発者, I want PR や push 時に CI が自動実行される, so that 手動でのチェック漏れを防ぎ、品質を継続的に担保できる

#### Acceptance Criteria
1. When main ブランチ向けの PR が作成または更新された場合, the CI workflow shall CI ジョブを自動実行する
2. When main ブランチへの push が行われた場合, the CI workflow shall CI ジョブを自動実行する

### Requirement 2: フォーマットチェック
**Objective:** As a 開発者, I want コードフォーマットが自動でチェックされる, so that コードスタイルの統一性を維持できる

#### Acceptance Criteria
1. When CI ジョブが実行された場合, the CI workflow shall `cargo fmt -- --check` によるフォーマットチェックを実行する
2. If フォーマット違反が検出された場合, the CI workflow shall ジョブを失敗ステータスで終了する

### Requirement 3: 静的解析（Lint）
**Objective:** As a 開発者, I want clippy による静的解析が自動実行される, so that 潜在的なバグやコード品質の問題を早期に検出できる

#### Acceptance Criteria
1. When CI ジョブが実行された場合, the CI workflow shall `cargo clippy --all-targets` による lint チェックを実行する
2. The CI workflow shall `RUSTFLAGS="-D warnings"` を設定し、全ての警告をエラーとして扱う
3. If clippy の警告またはエラーが検出された場合, the CI workflow shall ジョブを失敗ステータスで終了する

### Requirement 4: ユニットテスト実行
**Objective:** As a 開発者, I want ユニットテストが自動で実行される, so that リグレッションを早期に検出できる

#### Acceptance Criteria
1. When CI ジョブが実行された場合, the CI workflow shall `cargo test --lib -- --test-threads=1` によるユニットテストを実行する
2. If テストが失敗した場合, the CI workflow shall ジョブを失敗ステータスで終了する

### Requirement 5: ビルドキャッシュとパフォーマンス
**Objective:** As a 開発者, I want CI のビルド時間が最適化される, so that フィードバックループを短縮し開発効率を維持できる

#### Acceptance Criteria
1. The CI workflow shall rust-cache によるビルドキャッシュを有効化する
2. The CI workflow shall stable ツールチェインと rustfmt, clippy コンポーネントを使用する

### Requirement 6: ワークフロー定義ファイル
**Objective:** As a 開発者, I want CI 設定が標準的な場所に配置される, so that GitHub Actions が自動的にワークフローを認識・実行できる

#### Acceptance Criteria
1. The CI workflow shall `.github/workflows/ci.yml` としてワークフロー定義ファイルを配置する
2. The CI workflow shall カラー出力を有効化する環境変数 `CARGO_TERM_COLOR=always` を設定する
