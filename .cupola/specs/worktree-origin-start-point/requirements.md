# Requirements Document

## Introduction
現在の cupola の worktree 作成処理では、起点（start_point）としてローカルの `HEAD` を使用している。しかし、ローカルの main ブランチがリモートより古い場合、最新のコードではなく古いコミットから作業が始まるバグがある。本機能では、worktree 作成前に `git fetch origin` を実行し、起点を `origin/{default_branch}` に変更することで、常にリモートの最新コミットから作業を開始できるようにする。

## Requirements

### Requirement 1: リモート最新の取得
**Objective:** 開発者として、worktree 作成前にリモートの最新状態を取得したい。これにより、常に最新のコードを起点に作業を開始できる。

#### Acceptance Criteria
1. When Issue の初期化処理で worktree を作成する前に、Cupola shall `git fetch origin` を実行してリモートの最新情報を取得する
2. If `git fetch origin` が失敗した場合、then Cupola shall エラーを返し worktree の作成を中断する

### Requirement 2: worktree 起点の変更
**Objective:** 開発者として、worktree がリモートの default_branch の最新コミットを起点に作成されるようにしたい。これにより、ローカルの main が古い場合でも最新のコードから作業を開始できる。

#### Acceptance Criteria
1. When worktree を作成する際に、Cupola shall 起点として `origin/{default_branch}` を使用する（`HEAD` の代わりに）
2. When ローカルの main ブランチがリモートより古い場合でも、Cupola shall リモートの最新コミットを起点に worktree を作成する

### Requirement 3: GitWorktree trait の拡張
**Objective:** 開発者として、Git のリモート取得操作を trait 経由で抽象化したい。これにより、テスト容易性とクリーンアーキテクチャの原則を維持できる。

#### Acceptance Criteria
1. Cupola shall GitWorktree trait（または関連する port trait）に fetch 操作のメソッドを提供する
2. When fetch メソッドが呼び出された際に、Cupola shall 指定されたリモートから最新情報を取得する
