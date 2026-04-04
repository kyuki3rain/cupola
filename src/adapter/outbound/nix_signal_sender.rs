use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::application::port::signal::SignalPort;
use crate::application::stop_use_case::StopError;

/// POSIX シグナル（SIGTERM / SIGKILL）を nix クレート経由で送信する実装。
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
