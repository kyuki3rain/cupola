use std::time::Duration;

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::application::port::pid_file::{PidFileError, PidFilePort};

pub struct StopUseCase<P: PidFilePort> {
    pid_file: P,
    /// SIGTERM 送信後の終了待機タイムアウト（デフォルト 30 秒）
    shutdown_timeout: Duration,
}

impl<P: PidFilePort> StopUseCase<P> {
    pub fn new(pid_file: P, shutdown_timeout: Duration) -> Self {
        Self {
            pid_file,
            shutdown_timeout,
        }
    }

    /// 停止を実行する。成功時 Ok(StopResult)。
    pub async fn execute(&self) -> Result<StopResult, StopError> {
        let pid = match self.pid_file.read_pid()? {
            None => return Ok(StopResult::NotRunning),
            Some(p) => p,
        };

        if !self.pid_file.is_process_alive(pid) {
            self.pid_file.delete_pid()?;
            return Ok(StopResult::StalePidCleaned { pid });
        }

        // Send SIGTERM
        let nix_pid = Pid::from_raw(pid as i32);
        kill(nix_pid, Signal::SIGTERM).map_err(|e| StopError::Signal(e.to_string()))?;

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
                let _ = kill(nix_pid, Signal::SIGKILL);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPidFile {
        pid: Option<u32>,
        alive: bool,
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
            self.alive
        }
    }

    #[tokio::test]
    async fn returns_not_running_when_no_pid_file() {
        let mock = MockPidFile {
            pid: None,
            alive: false,
        };
        let use_case = StopUseCase::new(mock, Duration::from_secs(30));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::NotRunning);
    }

    #[tokio::test]
    async fn returns_stale_pid_cleaned_when_process_dead() {
        let mock = MockPidFile {
            pid: Some(99999),
            alive: false,
        };
        let use_case = StopUseCase::new(mock, Duration::from_secs(30));
        let result = use_case.execute().await.expect("execute");
        assert_eq!(result, StopResult::StalePidCleaned { pid: 99999 });
    }
}
