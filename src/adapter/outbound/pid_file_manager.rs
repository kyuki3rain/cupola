use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use nix::sys::signal::kill;
use nix::unistd::Pid;

use crate::application::port::pid_file::{PidFileError, PidFilePort, ProcessMode};

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
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.pid_file_path)
            .map_err(|e| {
                if e.kind() == ErrorKind::AlreadyExists {
                    PidFileError::AlreadyExists
                } else {
                    PidFileError::Write(e.to_string())
                }
            })?;
        writeln!(file, "{pid}").map_err(|e| PidFileError::Write(e.to_string()))
    }

    fn read_pid(&self) -> Result<Option<u32>, PidFileError> {
        if !self.pid_file_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&self.pid_file_path)
            .map_err(|e| PidFileError::Read(e.to_string()))?;
        let pid_str = content.lines().next().unwrap_or("").trim();
        let pid = pid_str
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

    fn write_pid_with_mode(&self, pid: u32, mode: ProcessMode) -> Result<(), PidFileError> {
        let mode_str = match mode {
            ProcessMode::Foreground => "foreground",
            ProcessMode::Daemon => "daemon",
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.pid_file_path)
            .map_err(|e| {
                if e.kind() == ErrorKind::AlreadyExists {
                    PidFileError::AlreadyExists
                } else {
                    PidFileError::Write(e.to_string())
                }
            })?;
        write!(file, "{pid}\n{mode_str}\n").map_err(|e| PidFileError::Write(e.to_string()))
    }

    fn read_pid_and_mode(&self) -> Result<Option<(u32, Option<ProcessMode>)>, PidFileError> {
        if !self.pid_file_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&self.pid_file_path)
            .map_err(|e| PidFileError::Read(e.to_string()))?;
        let mut lines = content.lines();
        let pid_str = lines.next().unwrap_or("");
        let pid = pid_str
            .parse::<u32>()
            .map_err(|e| PidFileError::InvalidContent(e.to_string()))?;
        if pid == 0 || pid > i32::MAX as u32 {
            return Err(PidFileError::InvalidContent(format!(
                "PID {pid} is out of valid range (1..={})",
                i32::MAX
            )));
        }
        let mode = match lines.next() {
            None => {
                tracing::info!("PID file has no mode line (legacy format), treating as unknown");
                None
            }
            Some("foreground") => Some(ProcessMode::Foreground),
            Some("daemon") => Some(ProcessMode::Daemon),
            Some(other) => {
                tracing::warn!(
                    mode = other,
                    "PID file contains unknown mode string, treating as unknown"
                );
                None
            }
        };
        Ok(Some((pid, mode)))
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
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("roundtrip.pid");
        let mgr = PidFileManager::new(path);

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
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("delete_test.pid");
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

    #[test]
    fn write_pid_returns_already_exists_on_second_call() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("double.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid(111).expect("first write should succeed");
        let err = mgr.write_pid(222).expect_err("second write should fail");
        assert!(
            matches!(err, PidFileError::AlreadyExists),
            "expected AlreadyExists, got {err:?}"
        );
    }

    #[test]
    fn read_pid_succeeds_on_two_line_pid_file() {
        // Regression test: read_pid() must parse only the first line of a 2-line PID file
        // written by write_pid_with_mode(). Previously, content.trim().parse() would fail
        // because "1234\nforeground" is not a valid u32.
        let tmp = NamedTempFile::new().expect("temp file");
        std::fs::write(tmp.path(), "12345\nforeground\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        let result = mgr
            .read_pid()
            .expect("read_pid should succeed on 2-line file");
        assert_eq!(result, Some(12345));
    }

    #[test]
    fn test_write_and_read_pid_with_mode_foreground() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("foreground.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid_with_mode(12345, ProcessMode::Foreground)
            .expect("write");
        let result = mgr.read_pid_and_mode().expect("read");
        assert_eq!(result, Some((12345, Some(ProcessMode::Foreground))));
    }

    #[test]
    fn test_write_and_read_pid_with_mode_daemon() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("daemon.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid_with_mode(99999, ProcessMode::Daemon)
            .expect("write");
        let result = mgr.read_pid_and_mode().expect("read");
        assert_eq!(result, Some((99999, Some(ProcessMode::Daemon))));
    }

    #[test]
    fn test_write_pid_with_mode_already_exists() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("double_mode.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid_with_mode(111, ProcessMode::Foreground)
            .expect("first write");
        let err = mgr
            .write_pid_with_mode(222, ProcessMode::Daemon)
            .expect_err("second write should fail");
        assert!(
            matches!(err, PidFileError::AlreadyExists),
            "expected AlreadyExists, got {err:?}"
        );
    }

    #[test]
    fn test_read_pid_and_mode_legacy_single_line() {
        let tmp = NamedTempFile::new().expect("temp file");
        std::fs::write(tmp.path(), "54321\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        let result = mgr.read_pid_and_mode().expect("read");
        assert_eq!(result, Some((54321, None)));
    }

    #[test]
    fn test_read_pid_and_mode_returns_none_when_absent() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("nonexistent_mode.pid");
        let mgr = PidFileManager::new(path);
        let result = mgr.read_pid_and_mode().expect("read");
        assert!(result.is_none());
    }

    #[test]
    fn test_read_pid_and_mode_invalid_pid_zero() {
        let tmp = NamedTempFile::new().expect("temp file");
        std::fs::write(tmp.path(), "0\nforeground\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        assert!(matches!(
            mgr.read_pid_and_mode(),
            Err(PidFileError::InvalidContent(_))
        ));
    }

    #[test]
    fn test_read_pid_and_mode_invalid_pid_overflow() {
        let tmp = NamedTempFile::new().expect("temp file");
        // i32::MAX + 1 = 2147483648
        std::fs::write(tmp.path(), "2147483648\ndaemon\n").expect("write");
        let mgr = manager_from_tempfile(&tmp);
        assert!(matches!(
            mgr.read_pid_and_mode(),
            Err(PidFileError::InvalidContent(_))
        ));
    }

    #[test]
    fn test_write_pid_with_mode_succeeds_after_delete() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("retry_mode.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid_with_mode(333, ProcessMode::Foreground)
            .expect("first write");
        mgr.delete_pid().expect("delete");
        mgr.write_pid_with_mode(444, ProcessMode::Daemon)
            .expect("write after delete");
        let result = mgr.read_pid_and_mode().expect("read");
        assert_eq!(result, Some((444, Some(ProcessMode::Daemon))));
    }

    #[test]
    fn write_pid_succeeds_after_delete_pid() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("retry.pid");
        let mgr = PidFileManager::new(path);

        mgr.write_pid(333).expect("first write");
        mgr.delete_pid().expect("delete");
        mgr.write_pid(444)
            .expect("write after delete should succeed");
        let pid = mgr.read_pid().expect("read");
        assert_eq!(pid, Some(444));
    }
}
