use crate::application::stop_use_case::StopError;

/// OS シグナル送信を抽象化する outbound ポート。
pub trait SignalPort: Send + Sync {
    fn send_sigterm(&self, pid: u32) -> Result<(), StopError>;
    fn send_sigkill(&self, pid: u32) -> Result<(), StopError>;
}
