# Implementation Plan

- [x] 1. Release ワークフローの基本構造とトリガー設定
  - ワークフローファイルを作成し、セマンティックバージョニング形式のタグ push でトリガーされるよう設定する
  - GitHub Actions の `on.push.tags` は glob として解釈されることに留意し、タグパターン `v[0-9]*.[0-9]*.[0-9]*` にマッチするイベントのみを対象とする
  - ワークフロー内のジョブ開始時に、`GITHUB_REF_NAME` などから取得したタグ名が正規表現 `^v[0-9]+\.[0-9]+\.[0-9]+$` にマッチするか検証し、マッチしない場合は失敗させるステップを追加する
  - パーミッションは contents: write のみを設定し、最小権限原則に従う
  - 環境変数 CARGO_TERM_COLOR: always を設定する
  - _Requirements: 1.1, 1.2, 1.3, 5.2_

- [x] 2. クロスコンパイルビルドジョブの実装
  - matrix strategy で 3 ターゲット（x86_64-unknown-linux-gnu, aarch64-apple-darwin, x86_64-apple-darwin）を定義する
  - 各ターゲットに適切な OS ランナー（ubuntu-latest / macos-latest）を割り当てる
  - fail-fast を無効にし、各ターゲットのビルドが独立して実行されるようにする
  - Rust stable ツールチェーンのインストールとターゲット追加を設定する
  - ターゲットごとのビルドキャッシュを設定する
  - cargo build --release でリリースビルドを実行する
  - 全 Action 参照はコミットハッシュでピン留めする
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 5.1_

- [x] 3. バイナリパッケージングとアーティファクトアップロード
  - ビルド成果物を cupola-{target}.tar.gz 形式でパッケージする
  - アーティファクトとしてアップロードし、ファイル不在時はエラーで失敗するよう設定する
  - Action 参照はコミットハッシュでピン留めする
  - _Requirements: 3.1, 3.2, 3.3, 5.1_

- [x] 4. GitHub Release 作成ジョブの実装
  - build ジョブへの依存（needs: build）を設定し、全ビルド完了後に実行されるようにする
  - 全ターゲットのアーティファクトをダウンロードする
  - GitHub Release を作成し、全 tar.gz ファイルを添付する
  - リリースノートの自動生成を有効にする
  - Action 参照はコミットハッシュでピン留めする
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.1_
