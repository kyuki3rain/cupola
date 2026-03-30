# Requirements Document

## Project Description (Input)
Release workflow に aarch64-unknown-linux-gnu（Linux ARM64）ターゲットを追加し、Raspberry Pi 5 等の ARM64 Linux デバイス向けバイナリをリリースビルドできるようにする

## はじめに

cupola は現在 x86_64 向けのリリースバイナリのみを提供しているが、Raspberry Pi 5 等の ARM64 Linux デバイス（aarch64-unknown-linux-gnu）への需要が生じている。本機能では GitHub Actions の Release workflow を新設し、タグ push をトリガーとして aarch64-unknown-linux-gnu を含む複数ターゲット向けバイナリを自動ビルド・リリースできるようにする。

cupola は rusqlite の `bundled` feature を使用しているため、クロスコンパイル時に C コンパイラのクロス設定（`gcc-aarch64-linux-gnu`）または `cross` crate によるコンテナビルドが必要となる。

## Requirements

### Requirement 1: Release ワークフローの新設

**Objective:** As a 開発者, I want タグ push 時に自動でリリースビルドが実行される GitHub Actions ワークフロー, so that 手動ビルド・アップロードの手間なく GitHub Release に複数ターゲットのバイナリを公開できる

#### Acceptance Criteria
1. When `v*.*.*` 形式のタグが push される, the Release Workflow shall `.github/workflows/release.yml` がトリガーされる
2. The Release Workflow shall `x86_64-unknown-linux-gnu` および `aarch64-unknown-linux-gnu` の少なくとも 2 ターゲットを matrix でビルドする
3. When ビルドが成功する, the Release Workflow shall 各ターゲット向けバイナリを `cupola-<target>.tar.gz` の形式でアーカイブする
4. When アーカイブが生成される, the Release Workflow shall 対応するタグの GitHub Release にアップロードする

### Requirement 2: aarch64-unknown-linux-gnu クロスコンパイル対応

**Objective:** As a 開発者, I want ubuntu-latest 上で aarch64-unknown-linux-gnu 向けクロスコンパイルが成功する環境, so that Raspberry Pi 5 等の ARM64 Linux デバイスで動作するバイナリが生成できる

#### Acceptance Criteria
1. The Release Workflow shall `aarch64-unknown-linux-gnu` Rust ターゲットをツールチェーンに追加する
2. The Release Workflow shall rusqlite の `bundled` feature のクロスコンパイルに必要な C クロスコンパイラ（`gcc-aarch64-linux-gnu`）をインストールする
3. When `aarch64-unknown-linux-gnu` をターゲットにビルドする, the Release Workflow shall `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` 環境変数を適切に設定する
4. If クロスコンパイルが失敗する, the Release Workflow shall ビルドジョブが失敗としてマークされ、後続のアップロードステップを実行しない

### Requirement 3: リリースアーティファクトの公開

**Objective:** As a ユーザー, I want GitHub Release から aarch64-unknown-linux-gnu バイナリを直接ダウンロードできる, so that Raspberry Pi 5 等の ARM64 Linux デバイスに cupola を簡単にインストールできる

#### Acceptance Criteria
1. When Release ワークフローが完了する, the Release Workflow shall GitHub Release に `cupola-aarch64-unknown-linux-gnu.tar.gz` が含まれる
2. The Release Workflow shall 各アーカイブにバイナリ単体（`cupola`）を含む
3. While GitHub Release が存在しない場合, the Release Workflow shall 新規 Release を自動作成してアーティファクトをアップロードする
4. The Release Workflow shall `x86_64-unknown-linux-gnu` のビルド・公開も維持し、既存ユーザーへの影響がないようにする
