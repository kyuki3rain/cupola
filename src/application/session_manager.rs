use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, ExitStatus};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::domain::state::State;

pub struct SessionManager {
    sessions: HashMap<i64, SessionEntry>,
}

struct SessionEntry {
    child: Child,
    started_at: Instant,
    registered_state: State,
    stdout_handle: Option<JoinHandle<String>>,
    stderr_handle: Option<JoinHandle<String>>,
    log_id: i64,
    /// ProcessRun id (Phase 3+); 0 if not yet set.
    run_id: i64,
}

pub struct ExitedSession {
    pub issue_id: i64,
    pub exit_status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub log_id: i64,
    /// ProcessRun id that owns this session (0 if not used).
    pub run_id: i64,
    /// The issue state at the time this session was registered.
    pub registered_state: State,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn register(&mut self, issue_id: i64, state: State, mut child: Child) {
        if let Some(mut old_entry) = self.sessions.remove(&issue_id) {
            let _ = old_entry.child.kill();
            let _ = old_entry.child.wait();
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_handle = stdout.map(|s| {
            std::thread::spawn(move || {
                let mut buf = String::new();
                let mut reader = std::io::BufReader::new(s);
                let _ = reader.read_to_string(&mut buf);
                buf
            })
        });

        let stderr_handle = stderr.map(|s| {
            std::thread::spawn(move || {
                let mut buf = String::new();
                let mut reader = std::io::BufReader::new(s);
                let _ = reader.read_to_string(&mut buf);
                buf
            })
        });

        self.sessions.insert(
            issue_id,
            SessionEntry {
                child,
                started_at: Instant::now(),
                registered_state: state,
                stdout_handle,
                stderr_handle,
                log_id: 0,
                run_id: 0,
            },
        );
    }

    pub fn collect_exited(&mut self) -> Vec<ExitedSession> {
        let mut exited_ids = Vec::new();

        for (&issue_id, entry) in &mut self.sessions {
            match entry.child.try_wait() {
                Ok(Some(_status)) => {
                    exited_ids.push(issue_id);
                }
                Ok(None) => {} // still running
                Err(e) => {
                    tracing::warn!(issue_id, error = %e, "try_wait failed");
                    exited_ids.push(issue_id);
                }
            }
        }

        let mut results = Vec::new();
        for issue_id in exited_ids {
            if let Some(mut entry) = self.sessions.remove(&issue_id) {
                #[expect(
                    clippy::expect_used,
                    reason = "wait after kill is practically unreachable"
                )]
                let exit_status = entry.child.try_wait().ok().flatten().unwrap_or_else(|| {
                    let _ = entry.child.kill();
                    entry.child.wait().expect("wait after kill")
                });

                let stdout = entry
                    .stdout_handle
                    .take()
                    .and_then(|h| h.join().ok())
                    .unwrap_or_default();

                let stderr = entry
                    .stderr_handle
                    .take()
                    .and_then(|h| h.join().ok())
                    .unwrap_or_default();

                results.push(ExitedSession {
                    issue_id,
                    exit_status,
                    stdout,
                    stderr,
                    log_id: entry.log_id,
                    run_id: entry.run_id,
                    registered_state: entry.registered_state,
                });
            }
        }

        results
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_running(&self, issue_id: i64) -> bool {
        self.sessions.contains_key(&issue_id)
    }

    pub fn kill(&mut self, issue_id: i64) -> anyhow::Result<()> {
        if let Some(entry) = self.sessions.get_mut(&issue_id) {
            entry.child.kill()?;
        }
        Ok(())
    }

    pub fn kill_all(&mut self) {
        for entry in self.sessions.values_mut() {
            let _ = entry.child.kill();
        }
    }

    pub fn update_log_id(&mut self, issue_id: i64, log_id: i64) {
        if let Some(entry) = self.sessions.get_mut(&issue_id) {
            entry.log_id = log_id;
        }
    }

    pub fn update_run_id(&mut self, issue_id: i64, run_id: i64) {
        if let Some(entry) = self.sessions.get_mut(&issue_id) {
            entry.run_id = run_id;
        }
    }

    pub fn find_stalled(&self, timeout: Duration) -> Vec<i64> {
        let now = Instant::now();
        self.sessions
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.started_at) > timeout)
            .map(|(&id, _)| id)
            .collect()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn spawn_echo() -> Child {
        Command::new("echo")
            .arg("hello from stdout")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("echo should work")
    }

    fn spawn_sleep(secs: u32) -> Child {
        Command::new("sleep")
            .arg(secs.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sleep should work")
    }

    #[test]
    fn register_and_is_running() {
        let mut mgr = SessionManager::new();
        let child = spawn_sleep(10);
        mgr.register(1, State::DesignRunning, child);

        assert!(mgr.is_running(1));
        assert!(!mgr.is_running(2));

        mgr.kill_all();
    }

    #[test]
    fn collect_exited_returns_finished_process() {
        let mut mgr = SessionManager::new();
        let child = spawn_echo();
        mgr.register(1, State::DesignRunning, child);

        // Wait for echo to finish
        std::thread::sleep(Duration::from_millis(200));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].issue_id, 1);
        assert!(exited[0].exit_status.success());
        assert!(exited[0].stdout.contains("hello from stdout"));
        assert!(!mgr.is_running(1));
    }

    #[test]
    fn collect_exited_skips_running_process() {
        let mut mgr = SessionManager::new();
        let child = spawn_sleep(10);
        mgr.register(1, State::DesignRunning, child);

        let exited = mgr.collect_exited();
        assert!(exited.is_empty());
        assert!(mgr.is_running(1));

        mgr.kill_all();
    }

    #[test]
    fn kill_stops_process() {
        let mut mgr = SessionManager::new();
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child);

        mgr.kill(1).expect("kill should work");

        // After kill, collect_exited should return it
        std::thread::sleep(Duration::from_millis(100));
        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert!(!exited[0].exit_status.success());
    }

    #[test]
    fn count_reflects_registered_and_exited() {
        let mut mgr = SessionManager::new();
        assert_eq!(mgr.count(), 0);

        let child1 = spawn_echo();
        mgr.register(1, State::DesignRunning, child1);
        assert_eq!(mgr.count(), 1);

        let child2 = spawn_sleep(10);
        mgr.register(2, State::ImplementationRunning, child2);
        assert_eq!(mgr.count(), 2);

        // Wait for echo to finish
        std::thread::sleep(Duration::from_millis(200));
        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert_eq!(mgr.count(), 1);

        mgr.kill_all();
    }

    #[test]
    fn collect_exited_includes_log_id() {
        let mut mgr = SessionManager::new();
        let child = spawn_echo();
        mgr.register(1, State::DesignRunning, child);
        mgr.update_log_id(1, 42);

        std::thread::sleep(Duration::from_millis(200));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].log_id, 42);
    }

    #[test]
    fn find_stalled_detects_old_processes() {
        let mut mgr = SessionManager::new();
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child);

        // With a very short timeout, everything is stalled
        let stalled = mgr.find_stalled(Duration::from_nanos(1));
        assert_eq!(stalled, vec![1]);

        // With a long timeout, nothing is stalled
        let stalled = mgr.find_stalled(Duration::from_secs(3600));
        assert!(stalled.is_empty());

        mgr.kill_all();
    }

    #[test]
    fn register_kills_old_process_on_duplicate_issue_id() {
        use nix::sys::signal;
        use nix::unistd::Pid;

        let mut mgr = SessionManager::new();

        // 最初のsleepプロセスを登録
        let old_child = spawn_sleep(60);
        let old_pid = old_child.id();
        mgr.register(1, State::DesignRunning, old_child);

        // 同一 issue_id に別プロセスを再登録すると旧プロセスがkillされ、wait()で回収される
        let new_child = spawn_sleep(60);
        mgr.register(1, State::ImplementationRunning, new_child);

        // register() 内で kill() + wait() が呼ばれるため、プロセスは既に回収済み。
        // kill(pid, 0) で ESRCH が返れば、プロセスが存在しないことを確認できる。
        let nix_pid = Pid::from_raw(old_pid as i32);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match signal::kill(nix_pid, None) {
                Err(nix::errno::Errno::ESRCH) => break, // プロセスは存在しない（正常）
                Ok(()) => {
                    // まだ存在する（終了処理中の可能性）
                    assert!(
                        Instant::now() < deadline,
                        "old process was not killed within timeout"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("unexpected kill(0) error: {e}"),
            }
        }

        mgr.kill_all();
    }
}
