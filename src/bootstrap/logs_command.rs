use std::io::{self, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow};

const TAIL_LINES: usize = 20;
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const LOG_PREFIX: &str = "cupola.";

/// Entry point for the `logs` command.
/// `log_dir`: value of `log.dir` from cupola.toml (None if not set)
/// `follow`: whether `-f` flag was provided
pub async fn run_logs(log_dir: Option<PathBuf>, follow: bool) -> Result<()> {
    let dir = log_dir.ok_or_else(|| anyhow!("log.dir が cupola.toml に設定されていません"))?;

    if !dir.exists() {
        return Err(anyhow!("ログディレクトリが存在しません: {}", dir.display()));
    }

    let latest = find_latest_log_file(&dir)?;
    let tail = read_tail_lines(&latest, TAIL_LINES)?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in &tail {
        writeln!(out, "{line}")?;
    }
    out.flush()?;

    if follow {
        let mut current_file = latest;
        let mut offset = std::fs::metadata(&current_file)?.len();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    return Ok(());
                }
                _ = tokio::time::sleep(POLL_INTERVAL) => {
                    // Check for log rotation (newer file)
                    if let Ok(Some(newer)) = find_newer_log_file(&dir, &current_file) {
                        current_file = newer;
                        offset = 0;
                    }

                    let (new_lines, new_offset) = read_new_lines(&current_file, offset).await?;
                    offset = new_offset;

                    if !new_lines.is_empty() {
                        let stdout = io::stdout();
                        let mut out = stdout.lock();
                        for line in &new_lines {
                            writeln!(out, "{line}")?;
                        }
                        out.flush()?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Find the latest log file (by lexicographic sort of filename) in `log_dir`.
/// Only files with the `cupola.` prefix are considered.
fn find_latest_log_file(log_dir: &Path) -> Result<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(log_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(LOG_PREFIX))
                    .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        return Err(anyhow!(
            "ログファイルが見つかりません: {}",
            log_dir.display()
        ));
    }

    entries.sort();
    Ok(entries.into_iter().last().unwrap())
}

/// Find a log file in `log_dir` with a name lexicographically greater than `current_file`.
/// Returns `None` if no newer file exists.
fn find_newer_log_file(log_dir: &Path, current_file: &Path) -> Result<Option<PathBuf>> {
    let current_name = current_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let mut newer: Vec<PathBuf> = std::fs::read_dir(log_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(LOG_PREFIX) && n > current_name)
                    .unwrap_or(false)
        })
        .collect();

    if newer.is_empty() {
        return Ok(None);
    }

    newer.sort();
    Ok(newer.into_iter().last())
}

/// Read the last `n` lines from a file (synchronously).
fn read_tail_lines(path: &Path, n: usize) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();

    if lines.len() <= n {
        Ok(lines)
    } else {
        Ok(lines[lines.len() - n..].to_vec())
    }
}

/// Read new bytes from `path` starting at `offset`, returning new lines and the updated offset.
async fn read_new_lines(path: &Path, offset: u64) -> Result<(Vec<String>, u64)> {
    let mut file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    let file_len = metadata.len();

    if file_len <= offset {
        return Ok((vec![], offset));
    }

    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncSeekExt;

    file.seek(SeekFrom::Start(offset)).await?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<String> = text.lines().map(|l| l.to_owned()).collect();

    Ok((lines, file_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_temp_dir() -> TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).expect("failed to write file");
        path
    }

    // ── find_latest_log_file ────────────────────────────────────────────

    #[test]
    fn find_latest_selects_newest_by_name() {
        let tmp = make_temp_dir();
        write_file(tmp.path(), "cupola.2026-03-30", "old");
        write_file(tmp.path(), "cupola.2026-04-01", "new");
        write_file(tmp.path(), "cupola.2026-03-31", "mid");

        let result = find_latest_log_file(tmp.path()).unwrap();
        assert_eq!(result.file_name().unwrap(), "cupola.2026-04-01");
    }

    #[test]
    fn find_latest_excludes_non_prefixed_files() {
        let tmp = make_temp_dir();
        write_file(tmp.path(), "cupola.2026-04-01", "log");
        write_file(tmp.path(), "other.2026-04-02", "other");

        let result = find_latest_log_file(tmp.path()).unwrap();
        assert_eq!(result.file_name().unwrap(), "cupola.2026-04-01");
    }

    #[test]
    fn find_latest_errors_when_no_log_files() {
        let tmp = make_temp_dir();
        write_file(tmp.path(), "not-a-log.txt", "content");

        let err = find_latest_log_file(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("ログファイルが見つかりません"));
    }

    #[test]
    fn find_latest_errors_on_empty_directory() {
        let tmp = make_temp_dir();
        let err = find_latest_log_file(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("ログファイルが見つかりません"));
    }

    // ── read_tail_lines ─────────────────────────────────────────────────

    #[test]
    fn read_tail_returns_all_lines_when_fewer_than_n() {
        let tmp = make_temp_dir();
        let content = (1..=10)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let path = write_file(tmp.path(), "cupola.2026-04-01", &content);

        let lines = read_tail_lines(&path, 20).unwrap();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[9], "line10");
    }

    #[test]
    fn read_tail_returns_last_n_lines_when_more_than_n() {
        let tmp = make_temp_dir();
        let content = (1..=30)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let path = write_file(tmp.path(), "cupola.2026-04-01", &content);

        let lines = read_tail_lines(&path, 20).unwrap();
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[0], "line11");
        assert_eq!(lines[19], "line30");
    }

    // ── find_newer_log_file ─────────────────────────────────────────────

    #[test]
    fn find_newer_returns_newer_file() {
        let tmp = make_temp_dir();
        let current = write_file(tmp.path(), "cupola.2026-04-01", "old");
        write_file(tmp.path(), "cupola.2026-04-02", "new");

        let result = find_newer_log_file(tmp.path(), &current).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "cupola.2026-04-02");
    }

    #[test]
    fn find_newer_returns_none_when_no_newer_file() {
        let tmp = make_temp_dir();
        let current = write_file(tmp.path(), "cupola.2026-04-01", "log");

        let result = find_newer_log_file(tmp.path(), &current).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_newer_excludes_non_prefixed_files() {
        let tmp = make_temp_dir();
        let current = write_file(tmp.path(), "cupola.2026-04-01", "log");
        write_file(tmp.path(), "other.2026-04-02", "other");

        let result = find_newer_log_file(tmp.path(), &current).unwrap();
        assert!(result.is_none());
    }
}
