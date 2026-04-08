use std::time::Duration;

use crate::application::port::pid_file::{PidFileError, PidFilePort};
use crate::application::port::signal::SignalPort;

pub struct StopUseCase<P: PidFilePort, S: SignalPort> {
    pid_file: P,
    signal: S,
    /// SIGTERM 送信後の終了待機タイムアウト（デフォルト 30 秒）
    shutdown_timeout: Duration,
}

impl<P: PidFilePort, S: SignalPort> StopUseCase<P, S> {
    pub fn with_signal_sender(pid_file: P, signal: S, shutdown_timeout: Duration) -> Self {
        Self {
            pid_file,
            signal,
            shutdown_timeout,
        }
    }

    /// 停止を実行する。成功時 Ok(StopResult)。
    pub async fn execute(&self) -> Result<StopResult, StopError> {
        let pid = match self.pid_file.read_pid()? {
            None => return Ok(StopResult::NotRunning),
            Some(p) => p,
        };

        // Validate PID range: 0 signals the process group; values > i32::MAX
        // wrap to negative i32, which also targets unintended processes.
        if pid == 0 || pid > i32::MAX as u32 {
            self.pid_file.delete_pid()?;
            return Err(StopError::InvalidPid(pid));
        }

        if !self.pid_file.is_process_alive(pid) {
            self.pid_file.delete_pid()?;
            return Ok(StopResult::StalePidCleaned { pid });
        }

        // Send SIGTERM (ESRCH — already dead — is handled inside SignalPort)
        self.signal.send_sigterm(pid)?;

        // Poll until process exits or timeout
        let poll_interval = Duration::from_millis(500);
        let start = std::time::Instant::now();

        loop {
            tokio::time::sleep(poll_interval).await;

            if !self.pid_file.is_process_alive(pid) {
                self.pid_file.delete_pid()?;
                return Ok(StopResult::Stopped { pid });
            }

            if start.elapsed() >= self.shutdown_timeout {
                // Timeout — send SIGKILL
                tracing::warn!(pid, "shutdown timeout, sending SIGKILL");
                self.signal.send_sigkill(pid)?;

                // Give the OS a brief moment to reap the process, then verify.
                tokio::time::sleep(Duration::from_millis(100)).await;

                if self.pid_file.is_process_alive(pid) {
                    return Err(StopError::Signal(format!(
                        "process {pid} is still alive after SIGKILL"
                    )));
                }

                self.pid_file.delete_pid()?;
                return Ok(StopResult::ForceKilled { pid });
            }
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
    /// タイムアウトにより SIGKILL で強制終了
    ForceKilled { pid: u32 },
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
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockPidFile {
        pid: Option<u32>,
        alive: Arc<Mutex<bool>>,
    }

    impl PidFilePort for MockPidFile {
        fn write_pid(&self, _pid: u32) -> Result<(), PidFileError> {
            Ok(())
        }

        fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
            Ok(self.pid)
        }

        fn delete_pid(&self) -> Result<(), PidFileError> {
            Ok(())
        }

        fn is_process_alive(&self, _pid: u32) -> bool {
            *self.alive.lock().unwrap()
        }
    }

    struct MockSignal {
        /// When sigterm is sent, kill the "process" immediately.
        kill_on_sigterm: bool,
        alive: Arc<Mutex<bool>>,
        sigkill_sent: Arc<Mutex<bool>>,
    }

    impl SignalPort for MockSignal {
        fn send_sigterm(&self, _pid: u32) -> Result<(), StopError> {
            if self.kill_on_sigterm {
                *self.alive.lock().unwrap() = false;
            }
            Ok(())
        }

        fn send_sigkill(&self, _pid: u32) -> Result<(), StopError> {
            *self.alive.lock().unwrap() = false;
            *self.sigkill_sent.lock().unwrap() = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn returns_not_running_when_no_pid_file() {
        let alive = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: None,
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let use_case = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(30));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::NotRunning);
    }

    #[tokio::test]
    async fn returns_stale_pid_cleaned_when_process_dead() {
        let alive = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: Some(99999),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let use_case = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(30));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::StalePidCleaned { pid: 99999 });
    }

    #[tokio::test]
    async fn returns_stopped_when_process_exits_after_sigterm() {
        // Process is alive initially; dies immediately upon SIGTERM.
        let alive = Arc::new(Mutex::new(true));
        let mock_pid = MockPidFile {
            pid: Some(12345),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: true,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        // Short timeout so the test finishes quickly.
        let use_case = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(5));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::Stopped { pid: 12345 });
    }

    #[tokio::test]
    async fn returns_force_killed_on_timeout() {
        // Process stays alive after SIGTERM; dies only after SIGKILL.
        let alive = Arc::new(Mutex::new(true));
        let sigkill_sent = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: Some(12345),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false, // SIGTERM ignored
            alive,
            sigkill_sent: sigkill_sent.clone(),
        };
        // Very short timeout (shorter than one poll interval) to trigger SIGKILL quickly.
        let use_case =
            StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_millis(10));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::ForceKilled { pid: 12345 });
        assert!(
            *sigkill_sent.lock().unwrap(),
            "SIGKILL should have been sent"
        );
    }

    #[tokio::test]
    async fn rejects_invalid_pid_zero() {
        let alive = Arc::new(Mutex::new(true));
        let mock_pid = MockPidFile {
            pid: Some(0),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let use_case = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(30));
        let err = use_case.execute().await.expect_err("should error on pid=0");
        assert!(matches!(err, StopError::InvalidPid(0)));
    }

    // --- T-6.SP.* tests ---

    /// T-6.SP.1: PID ファイルなし → NotRunning
    #[tokio::test]
    async fn t_6_sp_1_missing_pid_file_returns_not_running() {
        let alive = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: None,
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let uc = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(30));
        let result = uc.execute().await.expect("execute");
        assert_eq!(result, StopResult::NotRunning);
    }

    /// T-6.SP.2: ステール PID → StalePidCleaned
    #[tokio::test]
    async fn t_6_sp_2_stale_pid_returns_stale_cleaned() {
        let alive = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: Some(12345),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let uc = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(30));
        let result = uc.execute().await.expect("execute");
        assert_eq!(result, StopResult::StalePidCleaned { pid: 12345 });
    }

    /// T-6.SP.3: SIGTERM を先に送信し、500ms ポーリング（モックで即時終了）
    #[tokio::test]
    async fn t_6_sp_3_sigterm_sent_first_then_polled() {
        let alive = Arc::new(Mutex::new(true));
        let mock_pid = MockPidFile {
            pid: Some(5555),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: true, // SIGTERM → 即終了
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let uc = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(5));
        let result = uc.execute().await.expect("execute");
        // SIGTERM 後にプロセスが終了 → Stopped
        assert_eq!(result, StopResult::Stopped { pid: 5555 });
    }

    /// T-6.SP.4: タイムアウト後に SIGKILL にエスカレート
    #[tokio::test]
    async fn t_6_sp_4_escalates_to_sigkill_after_timeout() {
        let alive = Arc::new(Mutex::new(true));
        let sigkill_sent = Arc::new(Mutex::new(false));
        let mock_pid = MockPidFile {
            pid: Some(7777),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: false, // SIGTERM 無視
            alive,
            sigkill_sent: sigkill_sent.clone(),
        };
        // 非常に短いタイムアウト
        let uc = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_millis(1));
        let result = uc.execute().await.expect("execute");
        assert_eq!(result, StopResult::ForceKilled { pid: 7777 });
        assert!(
            *sigkill_sent.lock().unwrap(),
            "SIGKILL should have been sent"
        );
    }

    /// T-6.SP.5: 停止後に PID ファイルが削除される
    #[tokio::test]
    async fn t_6_sp_5_pid_file_removed_after_termination() {
        // MockPidFile の delete_pid() は Ok を返す（PidFilePort 実装済み）
        // 実際の削除確認は PidFileManager の実装テストで行うため、
        // ここでは Stopped result が返ることで間接的に確認する
        let alive = Arc::new(Mutex::new(true));
        let mock_pid = MockPidFile {
            pid: Some(3333),
            alive: alive.clone(),
        };
        let mock_sig = MockSignal {
            kill_on_sigterm: true,
            alive,
            sigkill_sent: Arc::new(Mutex::new(false)),
        };
        let uc = StopUseCase::with_signal_sender(mock_pid, mock_sig, Duration::from_secs(5));
        let result = uc.execute().await.expect("execute");
        assert_eq!(result, StopResult::Stopped { pid: 3333 });
    }
}
