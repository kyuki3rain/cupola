# Requirements Document

## Introduction
cupola に同時実行数制限（max_concurrent_sessions）を導入する。複数 Issue を同時処理する際の API rate limit やマシンリソース制約に対応し、安定動作を確保する。cupola.toml から設定可能にし、PollingUseCase の Step 7 でカウントチェックを行い、上限以上ならプロセス起動をスキップする。スキップされた Issue は次の polling サイクルで再試行される。

## Requirements

### Requirement 1: 同時実行数の設定
**Objective:** cupola の運用者として、同時実行セッション数の上限を設定ファイルで制御したい。API rate limit やマシンリソースに応じた安定運用のため。

#### Acceptance Criteria
1. Where max_concurrent_sessions が cupola.toml に設定されている場合, cupola shall 設定値を正の整数（1 以上）として読み込み、同時実行数の上限として適用する。0 以下の値が指定された場合はバリデーションエラーとする
2. Where max_concurrent_sessions が cupola.toml に設定されていない場合（フィールドが存在しない、または None の場合）, cupola shall 同時実行数を制限せずに動作する（後方互換性の維持）
3. The Config shall max_concurrent_sessions を Option<u32> 型のフィールドとして保持する

### Requirement 2: プロセス起動時の上限チェック
**Objective:** cupola の運用者として、同時実行中のプロセス数が上限に達している場合は新しいプロセスの起動を抑止したい。リソース過負荷を防止するため。

#### Acceptance Criteria
1. When PollingUseCase の Step 7（プロセス起動）が実行される際, cupola shall SessionManager の実行中プロセス数と max_concurrent_sessions を比較し、上限以上であれば新しいプロセスの起動をスキップする
2. When 実行中プロセス数が上限未満である場合, cupola shall 通常通りプロセスを起動する
3. While max_concurrent_sessions が未設定（None）の状態では, cupola shall 上限チェックを行わず全ての needs_process な Issue に対してプロセスを起動する

### Requirement 3: スキップされた Issue の再試行
**Objective:** cupola の運用者として、上限によりスキップされた Issue が自動的に再試行されることを保証したい。手動介入なしで全 Issue が処理されるため。

#### Acceptance Criteria
1. When 同時実行数上限によりプロセス起動がスキップされた場合, cupola shall Issue の状態を変更せず needs_process のまま維持する
2. When 次の polling サイクルで空きがある場合, cupola shall スキップされていた Issue のプロセスを起動する
3. The cupola shall キューイング機構を導入せず、既存の polling ループを再試行メカニズムとして利用する

### Requirement 4: SessionManager のカウント機能
**Objective:** cupola の開発者として、SessionManager から実行中プロセス数を取得したい。上限チェックや状態表示に利用するため。

#### Acceptance Criteria
1. The SessionManager shall 現在実行中のセッション数を返す count() メソッドを提供する
2. When count() が呼び出された場合, SessionManager shall 実行中プロセスの正確な数を usize 型で返す

### Requirement 5: 実行状態の可視化
**Objective:** cupola の運用者として、現在の実行中プロセス数と上限を確認したい。運用状況の把握のため。

#### Acceptance Criteria
1. When cupola status コマンドが実行された場合, cupola shall 現在の実行中プロセス数を表示する
2. Where max_concurrent_sessions が設定されている場合, cupola shall 上限値も合わせて表示する
