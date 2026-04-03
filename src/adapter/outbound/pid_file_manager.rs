use std::path::PathBuf;

use nix::sys::signal::kill;
use nix::unistd::Pid;

use crate::application::port::pid_file::{PidFileError, PidFilePort};

pub struct PidFileManager {
    pid_file_path: PathBuf,
}

impl PidFileManager {
    pub fn new(pid_file_path: PathBuf) -> Self {
        Self { pid_file_path }
    }
}

impl PidFilePort for PidFileManager {
    fn write_pid(&self, pid: u32) -> Result<(), PidFileError> {
        let content = format!("{pid}\n");
        std::fs::write(&self.pid_file_path, content).map_err(|e| PidFileError::Write(e.to_string()))
    }

    fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
        if !self.pid_file_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&self.pid_file_path)
            .map_err(|e| PidFileError::Read(e.to_string()))?;
        let pid = content
            .trim()
            .parse::<u32>()
            .map_err(|e| PidFileError::InvalidContent(e.to_string()))?;
        // PID 0 signals the process group; PIDs > i32::MAX wrap to negative values
        // when cast to i32, which would send signals to unintended targets.
        if pid == 0 || pid > i32::MAX as u32 {
            return Err(PidFileError::InvalidContent(format!(
                "PID {pid} is out of valid range (1..={})",
                i32::MAX
            )));
        }
        Ok(Some(pid))
    }

    fn delete_pid(&self) -> Result<(), PidFileError> {
        if !self.pid_file_path.exists() {
            return Ok(());
        }
        std::fs::remove_file(&self.pid_file_path).map_err(|e| PidFileError::Delete(e.to_string()))
    }

    fn is_process_alive(&self, pid: u32) -> bool {
        let nix_pid = Pid::from_raw(pid as i32);
        match kill(nix_pid, None) {
            Ok(()) => true,
            Err(nix::errno::Errno::EPERM) => true, // process exists but we lack permission
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn manager_from_tempfile(file: &NamedTempFile) -> PidFileManager {
        PidFileManager::new(file.path().to_path_buf())
    }

    #[test]
    fn write_and_read_pid_roundtrip() {
        let tmp = NamedTempFile::new().expect("temp file");
        let mgr = manager_from_tempfile(&tmp);

        mgr.write_pid(12345).expect("write");
        let pid = mgr.read_pid().expect("read");
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn read_pid_returns_none_when_file_absent() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("nonexistent.pid");
        let mgr = PidFileManager::new(path);
        let result = mgr.read_pid().expect("read");
        assert!(result.is_none());
    }

    #[test]
    fn delete_pid_removes_file() {
        let tmp = NamedTempFile::new().expect("temp file");
        let path = tmp.path().to_path_buf();
        let mgr = PidFileManager::new(path.clone());

        mgr.write_pid(999).expect("write");
        assert!(path.exists());

        mgr.delete_pid().expect("delete");
        assert!(!path.exists());
    }

    #[test]
    fn delete_pid_ok_when_file_absent() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("nonexistent_delete.pid");
        let mgr = PidFileManager::new(path);
        mgr.delete_pid()
            .expect("delete should succeed even if absent");
    }

    #[test]
    fn read_pid_returns_none_after_delete_pid() {
        // Simulates the "next startup" scenario: after a normal daemon shutdown that
        // deletes the PID file, read_pid() must return Ok(None) so the next startup
        // does not trigger stale-PID cleanup.
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("cupola.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid(12345).expect("write");
        mgr.delete_pid().expect("delete");

        let result = mgr.read_pid().expect("read_pid should succeed");
        assert!(
            result.is_none(),
            "read_pid should return Ok(None) after the PID file has been deleted"
        );
    }

    #[test]
    fn is_process_alive_returns_true_for_self() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let mgr = PidFileManager::new(dir.path().join("alive.pid"));
        let my_pid = std::process::id();
        assert!(mgr.is_process_alive(my_pid));
    }

    #[test]
    fn is_process_alive_returns_false_for_invalid_pid() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let mgr = PidFileManager::new(dir.path().join("dead.pid"));
        // u32::MAX as i32 is -1, which targets all processes — use a specific large invalid PID
        // PID 0 would signal the whole process group; use u32::MAX - 1 which is an invalid PID
        assert!(!mgr.is_process_alive(2_147_483_647)); // i32::MAX, very unlikely to be valid
    }

    #[test]
    fn read_pid_rejects_zero() {
        let tmp = NamedTempFile::new().expect("temp file");
        std::fs::write(tmp.path(), "0\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        assert!(matches!(
            mgr.read_pid(),
            Err(PidFileError::InvalidContent(_))
        ));
    }

    #[test]
    fn read_pid_rejects_overflow() {
        let tmp = NamedTempFile::new().expect("temp file");
        // i32::MAX + 1 = 2147483648
        std::fs::write(tmp.path(), "2147483648\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        assert!(matches!(
            mgr.read_pid(),
            Err(PidFileError::InvalidContent(_))
        ));
    }
}
