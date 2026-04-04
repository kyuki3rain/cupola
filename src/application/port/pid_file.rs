/// PID ファイル操作の抽象インターフェース
pub trait PidFilePort: Send + Sync {
    /// PID ファイルに pid を書き込む。
    fn write_pid(&self, pid: u32) -> Result<(), PidFileError>;

    /// PID ファイルから pid を読み込む。ファイルが存在しない場合は None。
    fn read_pid(&self) -> Result<Option<u32>, PidFileError>;

    /// PID ファイルを削除する。ファイルが存在しない場合は Ok。
    fn delete_pid(&self) -> Result<(), PidFileError>;

    /// 指定 PID のプロセスが生存しているか確認する。
    fn is_process_alive(&self, pid: u32) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum PidFileError {
    #[error("failed to write PID file: {0}")]
    Write(String),
    #[error("failed to read PID file: {0}")]
    Read(String),
    #[error("failed to delete PID file: {0}")]
    Delete(String),
    #[error("PID file contains invalid content: {0}")]
    InvalidContent(String),
    #[error("PID file already exists (cupola is already running)")]
    AlreadyExists,
}
