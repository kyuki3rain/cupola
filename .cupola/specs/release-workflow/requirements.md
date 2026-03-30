# Requirements Document

## Introduction
GitHub Actions の Release workflow を追加し、v* タグの push をトリガーとしてクロスプラットフォームのバイナリビルド・パッケージング・GitHub Release 作成を自動化する。OSS 配布に必要な Linux/macOS 向けバイナリの自動リリースパイプラインを構築する。

## Requirements

### Requirement 1: ワークフロートリガー
**Objective:** As a 開発者, I want セマンティックバージョニングタグの push で自動的にリリースワークフローが起動すること, so that 手動操作なしでリリースプロセスを開始できる

#### Acceptance Criteria
1. When タグ（glob: v[0-9]*.[0-9]*.[0-9]*）が push される, the Release workflow shall ビルドジョブを開始する（この glob にマッチするタグ push でワークフローが起動し、実際のセマンティックバージョニング形式の判定は後続ステップの正規表現検証で行う）
2. The Release workflow shall ワークフロー内でタグ名を正規表現（^v[0-9]+\.[0-9]+\.[0-9]+$）で検証し、一致しないタグの場合はエラーで失敗する
3. The Release workflow shall contents: write パーミッションを持つ

### Requirement 2: クロスコンパイルビルド
**Objective:** As a ユーザー, I want Linux および macOS 向けのバイナリが自動ビルドされること, so that 各プラットフォームで cupola を利用できる

#### Acceptance Criteria
1. When ワークフローがトリガーされる, the Release workflow shall x86_64-unknown-linux-gnu ターゲットでバイナリをビルドする
2. When ワークフローがトリガーされる, the Release workflow shall aarch64-apple-darwin ターゲットでバイナリをビルドする
3. When ワークフローがトリガーされる, the Release workflow shall x86_64-apple-darwin ターゲットでバイナリをビルドする
4. The Release workflow shall 各ターゲットに適切な OS ランナー（ubuntu-latest または macos-latest）を使用する
5. The Release workflow shall fail-fast を無効にし、1つのターゲットの失敗が他のビルドをキャンセルしないようにする
6. The Release workflow shall Rust stable ツールチェーンを使用する
7. The Release workflow shall ビルドキャッシュ（rust-cache）をターゲットごとに設定する

### Requirement 3: バイナリパッケージング
**Objective:** As a ユーザー, I want ビルドされたバイナリが tar.gz 形式でパッケージされること, so that 簡単にダウンロード・展開して利用できる

#### Acceptance Criteria
1. When ビルドが成功する, the Release workflow shall バイナリを cupola-{target}.tar.gz 形式でパッケージする
2. When パッケージが作成される, the Release workflow shall アーティファクトとしてアップロードする
3. If パッケージファイルが存在しない, the Release workflow shall エラーで失敗する（if-no-files-found: error）

### Requirement 4: GitHub Release 作成
**Objective:** As a 開発者, I want GitHub Release が自動作成されバイナリが添付されること, so that ユーザーがリリースページからバイナリをダウンロードできる

#### Acceptance Criteria
1. When 全ターゲットのビルドが完了する, the Release workflow shall GitHub Release を作成する
2. When GitHub Release を作成する, the Release workflow shall 全ターゲットの tar.gz ファイルを添付する
3. When GitHub Release を作成する, the Release workflow shall リリースノートを自動生成する
4. The Release workflow shall ビルドジョブの完了後にのみリリースジョブを実行する（needs: build）

### Requirement 5: セキュリティとベストプラクティス
**Objective:** As a メンテナー, I want ワークフローがセキュリティのベストプラクティスに従うこと, so that サプライチェーン攻撃のリスクを低減できる

#### Acceptance Criteria
1. The Release workflow shall 全ての uses アクションでコミットハッシュによるピン留めを使用する（タグ指定ではなく）
2. The Release workflow shall 必要最小限のパーミッション（contents: write）のみを設定する
