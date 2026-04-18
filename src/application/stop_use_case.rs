use std::time::Duration;

use crate::application::port::pid_file::{PidFileError, PidFilePort};
use crate::application::port::signal::SignalPort;

pub struct StopUseCase<P: PidFilePort, S: SignalPort> {
    pid_file: P,
    signal: S,
    /// Graceful 停止時の終了待機タイムアウト（`None` = 無限待機）。
    shutdown_timeout: Option<Duration>,
}

impl<P: PidFilePort, S: SignalPort> StopUseCase<P, S> {
    pub fn with_signal_sender(pid_file: P, signal: S, shutdown_timeout: Option<Duration>) -> Self {
        Self {
            pid_file,
            signal,
            shutdown_timeout,
        }
    }

    /// シグナルエラー後にプロセスが死亡している場合、PID ファイルを削除する。
    fn cleanup_pid_file_if_dead(&self, pid: u32) {
        if !self.pid_file.is_process_alive(pid)
            && let Err(de) = self.pid_file.delete_pid()
        {
            tracing::warn!(pid, error = %de, "failed to delete PID file after signal error");
        }
    }

    /// 停止を実行する。
    ///
    /// - `force = false`: SIGTERM → ポーリング → タイムアウト後 SIGKILL（graceful）
    /// - `force = true`: セッション数取得 → SIGKILL（強制終了）
    pub async fn execute(&self, force: bool) -> Result<StopResult, StopError> {
        let pid = match self.pid_file.read_pid()? {
            None => return Ok(StopResult::NotRunning),
            Some(p) => p,
        };

        // PID 範囲チェック
        if pid == 0 || pid > i32::MAX as u32 {
            self.pid_file.delete_pid()?;
            return Err(StopError::InvalidPid(pid));
        }

        if !self.pid_file.is_process_alive(pid) {
            self.pid_file.delete_pid()?;
            return Ok(StopResult::StalePidCleaned { pid });
        }

        if force {
            // 強制終了: SIGKILL 送信前にセッション状態ファイルを読み取る
            let sessions_killed = self.pid_file.read_session_count().unwrap_or(0);

            if let Err(e) = self.signal.send_sigkill(pid) {
                self.cleanup_pid_file_if_dead(pid);
                return Err(e);
            }

            // OS がプロセスを回収するのを少し待つ
            tokio::time::sleep(Duration::from_millis(200)).await;

            // PID ファイル削除（best-effort）
            let _ = self.pid_file.delete_pid();

            return Ok(StopResult::ForceKilled {
                pid,
                sessions_killed,
            });
        }

        // Graceful 停止: SIGTERM 送信
        if let Err(e) = self.signal.send_sigterm(pid) {
            self.cleanup_pid_file_if_dead(pid);
            return Err(e);
        }

        // プロセス終了を待機
        let poll_interval = Duration::from_millis(500);
        let start = std::time::Instant::now();
        let mut last_progress_report = std::time::Instant::now();

        loop {
            tokio::time::sleep(poll_interval).await;

            if !self.pid_file.is_process_alive(pid) {
                self.pid_file.delete_pid()?;
                return Ok(StopResult::Stopped { pid });
            }

            // 5 秒ごとに進捗表示
            if last_progress_report.elapsed() >= Duration::from_secs(5) {
                let sessions = self.pid_file.read_session_count().unwrap_or(0);
                let elapsed = start.elapsed().as_secs();
                tracing::info!(
                    sessions_remaining = sessions,
                    elapsed_secs = elapsed,
                    "waiting for daemon to stop..."
                );
                last_progress_report = std::time::Instant::now();
            }

            // タイムアウトチェック
            if let Some(timeout) = self.shutdown_timeout
                && start.elapsed() >= timeout
            {
                tracing::warn!(pid, "shutdown timeout, sending SIGKILL");
                if let Err(e) = self.signal.send_sigkill(pid) {
                    self.cleanup_pid_file_if_dead(pid);
                    return Err(e);
                }

                tokio::time::sleep(Duration::from_millis(100)).await;

                if self.pid_file.is_process_alive(pid) {
                    return Err(StopError::Signal(format!(
                        "process {pid} is still alive after SIGKILL"
                    )));
                }

                self.pid_file.delete_pid()?;
                return Ok(StopResult::ForceKilled {
                    pid,
                    sessions_killed: 0,
                });
            }
            // shutdown_timeout = None の場合はタイムアウトなし（無限待機）
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum StopResult {
    /// 正常停止（pid=N）
    Stopped { pid: u32 },
    /// ステール PID を削除して正常終了
    StalePidCleaned { pid: u32 },
    /// 起動していなかった
    NotRunning,
    /// SIGKILL で強制終了（sessions_killed = 終了セッション数）
    ForceKilled { pid: u32, sessions_killed: u32 },
}

#[derive(Debug, thiserror::Error)]
pub enum StopError {
    #[error("PID file error: {0}")]
    PidFile(#[from] PidFileError),
    #[error("failed to send signal: {0}")]
    Signal(String),
    #[error("PID file contains invalid PID: {0}")]
    InvalidPid(u32),
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use super::*;
    use crate::application::port::pid_file::{PidFileError, PidFilePort, ProcessMode};

    // -----------------------------------------------------------------------
    // Mock PidFilePort
    // -----------------------------------------------------------------------

    struct MockPidFile {
        pid: Option<u32>,
        alive: bool,
        deleted: AtomicBool,
        session_count: Option<u32>,
    }

    impl MockPidFile {
        fn new(pid: Option<u32>, alive: bool) -> Self {
            Self {
                pid,
                alive,
                deleted: AtomicBool::new(false),
                session_count: None,
            }
        }

        fn with_session_count(mut self, n: u32) -> Self {
            self.session_count = Some(n);
            self
        }

        fn was_deleted(&self) -> bool {
            self.deleted.load(Ordering::SeqCst)
        }
    }

    impl PidFilePort for MockPidFile {
        fn write_pid(&self, _: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
            Ok(self.pid)
        }

        fn delete_pid(&self) -> Result<(), PidFileError> {
            self.deleted.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn is_process_alive(&self, _: u32) -> bool {
            self.alive
        }

        fn write_pid_with_mode(&self, _: u32, _: ProcessMode) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid_and_mode(&self) -> Result<Option<(u32, Option<ProcessMode>)>, PidFileError> {
            Ok(self.pid.map(|p| (p, None)))
        }

        fn write_session_count(&self, _: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_session_count(&self) -> Option<u32> {
            self.session_count
        }

        fn delete_session_file(&self) -> Result<(), PidFileError> {
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Mock SignalPort
    // -----------------------------------------------------------------------

    struct MockSignal {
        sigterm_called: AtomicU32,
        sigkill_called: AtomicU32,
        sigterm_err: Option<String>,
        sigkill_err: Option<String>,
    }

    impl MockSignal {
        fn new() -> Self {
            Self {
                sigterm_called: AtomicU32::new(0),
                sigkill_called: AtomicU32::new(0),
                sigterm_err: None,
                sigkill_err: None,
            }
        }

        fn sigterm_count(&self) -> u32 {
            self.sigterm_called.load(Ordering::SeqCst)
        }

        fn sigkill_count(&self) -> u32 {
            self.sigkill_called.load(Ordering::SeqCst)
        }
    }

    impl SignalPort for MockSignal {
        fn send_sigterm(&self, _pid: u32) -> Result<(), StopError> {
            self.sigterm_called.fetch_add(1, Ordering::SeqCst);
            if let Some(ref e) = self.sigterm_err {
                return Err(StopError::Signal(e.clone()));
            }
            Ok(())
        }

        fn send_sigkill(&self, _pid: u32) -> Result<(), StopError> {
            self.sigkill_called.fetch_add(1, Ordering::SeqCst);
            if let Some(ref e) = self.sigkill_err {
                return Err(StopError::Signal(e.clone()));
            }
            Ok(())
        }
    }

    // プロセスが最初の is_process_alive 呼び出しでは alive、2回目以降は dead を返すモック
    // StopUseCase の graceful path テスト用（1回目: alive チェック通過 → SIGTERM → 2回目: dead）
    struct MockPidFileAliveThenDead {
        pid: u32,
        deleted: AtomicBool,
        alive_call_count: std::sync::atomic::AtomicUsize,
    }

    impl MockPidFileAliveThenDead {
        fn new(pid: u32) -> Self {
            Self {
                pid,
                deleted: AtomicBool::new(false),
                alive_call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl PidFilePort for MockPidFileAliveThenDead {
        fn write_pid(&self, _: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
            Ok(Some(self.pid))
        }

        fn delete_pid(&self) -> Result<(), PidFileError> {
            self.deleted.store(true, Ordering::SeqCst);
            Ok(())
        }

        // 最初の呼び出しは alive（stale チェック通過）、それ以降は dead（SIGTERM 後）
        fn is_process_alive(&self, _: u32) -> bool {
            let prev = self
                .alive_call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            prev == 0 // 0回目(1回目の呼び出し)のみ alive
        }

        fn write_pid_with_mode(&self, _: u32, _: ProcessMode) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid_and_mode(&self) -> Result<Option<(u32, Option<ProcessMode>)>, PidFileError> {
            Ok(Some((self.pid, None)))
        }

        fn write_session_count(&self, _: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_session_count(&self) -> Option<u32> {
            None
        }

        fn delete_session_file(&self) -> Result<(), PidFileError> {
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Tests: execute(false) — graceful
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn execute_false_returns_not_running_when_no_pid() {
        let pid_file = MockPidFile::new(None, false);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(false).await.expect("should succeed");
        assert_eq!(result, StopResult::NotRunning);
    }

    #[tokio::test]
    async fn execute_false_returns_stale_when_process_dead() {
        let pid_file = MockPidFile::new(Some(1234), false);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(false).await.expect("should succeed");
        assert!(matches!(result, StopResult::StalePidCleaned { pid: 1234 }));
        assert!(uc.pid_file.was_deleted());
    }

    #[tokio::test]
    async fn execute_false_sends_sigterm_not_sigkill_for_graceful() {
        // プロセスが最初の is_process_alive で dead を返すケース（SIGTERM 後すぐ終了）
        let pid_file = MockPidFileAliveThenDead::new(5678);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(false).await.expect("should succeed");
        assert!(matches!(result, StopResult::Stopped { pid: 5678 }));
        assert_eq!(uc.signal.sigterm_count(), 1);
        assert_eq!(uc.signal.sigkill_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Tests: execute(true) — force
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn execute_true_sends_sigkill_not_sigterm() {
        let pid_file = MockPidFile::new(Some(9999), true);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(true).await.expect("should succeed");
        assert_eq!(uc.signal.sigterm_count(), 0, "SIGTERM should NOT be sent");
        assert_eq!(uc.signal.sigkill_count(), 1, "SIGKILL should be sent");
        assert!(matches!(
            result,
            StopResult::ForceKilled {
                pid: 9999,
                sessions_killed: 0
            }
        ));
    }

    #[tokio::test]
    async fn execute_true_includes_session_count() {
        let pid_file = MockPidFile::new(Some(1111), true).with_session_count(3);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(true).await.expect("should succeed");
        assert!(matches!(
            result,
            StopResult::ForceKilled {
                pid: 1111,
                sessions_killed: 3
            }
        ));
    }

    #[tokio::test]
    async fn execute_true_returns_not_running_when_no_pid() {
        let pid_file = MockPidFile::new(None, false);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(true).await.expect("should succeed");
        assert_eq!(result, StopResult::NotRunning);
        assert_eq!(uc.signal.sigkill_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Tests: invalid PID
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn execute_rejects_pid_zero() {
        let pid_file = MockPidFile::new(Some(0), false);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(false).await;
        assert!(matches!(result, Err(StopError::InvalidPid(0))));
    }

    #[tokio::test]
    async fn execute_true_stale_pid_returns_stale_cleaned() {
        let pid_file = MockPidFile::new(Some(7777), false);
        let signal = MockSignal::new();
        let uc = StopUseCase::with_signal_sender(pid_file, signal, Some(Duration::from_secs(30)));
        let result = uc.execute(true).await.expect("should succeed");
        assert!(matches!(result, StopResult::StalePidCleaned { pid: 7777 }));
        assert_eq!(uc.signal.sigkill_count(), 0);
    }
}
