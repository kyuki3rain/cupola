use std::time::Duration;

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::application::port::pid_file::{PidFileError, PidFilePort};

/// Abstracts signal delivery so that `StopUseCase` can be unit-tested
/// without sending real OS signals.
pub trait SignalPort: Send + Sync {
    fn send_sigterm(&self, pid: u32) -> Result<(), StopError>;
    fn send_sigkill(&self, pid: u32) -> Result<(), StopError>;
}

/// Production implementation using nix.
pub struct NixSignalSender;

impl SignalPort for NixSignalSender {
    fn send_sigterm(&self, pid: u32) -> Result<(), StopError> {
        let nix_pid = Pid::from_raw(pid as i32);
        match kill(nix_pid, Signal::SIGTERM) {
            Ok(()) => Ok(()),
            // ESRCH means the process already exited — treat as success.
            Err(nix::errno::Errno::ESRCH) => Ok(()),
            Err(e) => Err(StopError::Signal(e.to_string())),
        }
    }

    fn send_sigkill(&self, pid: u32) -> Result<(), StopError> {
        let nix_pid = Pid::from_raw(pid as i32);
        kill(nix_pid, Signal::SIGKILL).map_err(|e| StopError::Signal(e.to_string()))
    }
}

pub struct StopUseCase<P: PidFilePort, S: SignalPort> {
    pid_file: P,
    signal: S,
    /// SIGTERM 送信後の終了待機タイムアウト（デフォルト 30 秒）
    shutdown_timeout: Duration,
}

impl<P: PidFilePort> StopUseCase<P, NixSignalSender> {
    pub fn new(pid_file: P, shutdown_timeout: Duration) -> Self {
        Self {
            pid_file,
            signal: NixSignalSender,
            shutdown_timeout,
        }
    }
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
}
