use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, LineWriter};
use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::domain::state::State;

const LOG_DIR: &str = ".cupola/logs/process-runs";


/// アクティブな子プロセスの PID 共有レジストリ。
/// SessionManager と panic hook の間でプロセス PID を共有する。
/// Arc<Mutex<_>> でラップされているため、clone() で安価に複数の所有者に配布できる。
#[derive(Clone, Default)]
pub struct ChildProcessRegistry {
    pids: Arc<Mutex<HashSet<u32>>>,
}

impl ChildProcessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 子プロセスが起動したときに PID を登録する。
    pub fn register(&self, pid: u32) {
        let mut set = match self.pids.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        set.insert(pid);
    }

    /// 子プロセスが終了・強制終了したときに PID を削除する。
    pub fn unregister(&self, pid: u32) {
        let mut set = match self.pids.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        set.remove(&pid);
    }

    #[cfg(test)]
    pub fn contains_pid(&self, pid: u32) -> bool {
        let set = match self.pids.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        set.contains(&pid)
    }

    #[cfg(test)]
    pub fn pids_is_empty(&self) -> bool {
        let set = match self.pids.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        set.is_empty()
    }

    /// Poison the internal Mutex by panicking while holding the lock.
    /// Intended only for tests that verify poisoned-mutex recovery.
    #[cfg(test)]
    pub fn poison_for_test(&self) {
        let clone = self.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = clone.pids.lock().expect("lock");
            panic!("intentional mutex poison");
        }));
    }

    /// 全登録プロセスへ SIGTERM → タイムアウト → SIGKILL を同期実行する。
    /// panic hook から呼ばれることを想定しており、async/await を使わない。
    pub fn shutdown_sync(&self, sigterm_timeout: Duration) {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        let pids: Vec<u32> = {
            let set = match self.pids.lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            set.iter().copied().collect()
        };

        if pids.is_empty() {
            return;
        }

        for &pid in &pids {
            if let Ok(raw) = i32::try_from(pid)
                && raw > 0
            {
                let _ = kill(Pid::from_raw(raw), Signal::SIGTERM);
            }
        }

        let poll_interval = Duration::from_millis(100);
        let deadline = Instant::now() + sigterm_timeout;

        loop {
            std::thread::sleep(poll_interval);

            let alive: Vec<u32> = pids
                .iter()
                .copied()
                .filter(|&pid| {
                    i32::try_from(pid)
                        .ok()
                        .filter(|&raw| raw > 0)
                        .map(|raw| kill(Pid::from_raw(raw), None).is_ok())
                        .unwrap_or(false)
                })
                .collect();

            if alive.is_empty() {
                return;
            }

            if Instant::now() >= deadline {
                for &pid in &alive {
                    if let Ok(raw) = i32::try_from(pid)
                        && raw > 0
                    {
                        let _ = kill(Pid::from_raw(raw), Signal::SIGKILL);
                    }
                }
                return;
            }
        }
    }
}

pub struct SessionManager {
    sessions: HashMap<i64, SessionEntry>,
    /// Slots reserved via [`try_reserve`] but not yet converted to active sessions.
    ///
    /// This counter prevents TOCTOU races between the session-limit check and the
    /// actual [`register`] call: concurrent `spawn_process` invocations (if the
    /// polling loop is ever parallelised) both increment `pending` atomically, so
    /// the combined count `sessions.len() + pending` stays within the configured
    /// limit even while async I/O is in flight.
    pending: usize,
    log_dir: PathBuf,
    registry: Option<ChildProcessRegistry>,
}

struct SessionEntry {
    child: Child,
    started_at: Instant,
    registered_state: State,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    stdout_reader_handle: Option<JoinHandle<()>>,
    stderr_reader_handle: Option<JoinHandle<()>>,
    log_id: i64,
    run_id: i64,
}

pub struct ExitedSession {
    pub issue_id: i64,
    pub exit_status: ExitStatus,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub log_id: i64,
    /// ProcessRun id that owns this session (0 if not used).
    pub run_id: i64,
    /// The issue state at the time this session was registered.
    pub registered_state: State,
}

fn start_reader_thread<R: std::io::Read + Send + 'static>(
    source: Option<R>,
    path: PathBuf,
) -> Option<JoinHandle<()>> {
    source.map(|s| {
        std::thread::spawn(move || {
            let file = match std::fs::File::create(&path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!(
                        path = %path.display(),
                        error = %e,
                        "failed to create process log file, output will not be saved"
                    );
                    // Drain the pipe to prevent the process from blocking on full pipe buffer
                    let _ = std::io::copy(&mut std::io::BufReader::new(s), &mut std::io::sink());
                    return;
                }
            };
            let mut writer = LineWriter::new(BufWriter::new(file));
            let mut reader = std::io::BufReader::new(s);
            if let Err(e) = std::io::copy(&mut reader, &mut writer) {
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "failed to write process output to log file"
                );
                // Keep draining the pipe so the child process cannot block on a full buffer.
                let _ = std::io::copy(&mut reader, &mut std::io::sink());
            }
        })
    })
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            pending: 0,
            log_dir: PathBuf::from(LOG_DIR),
            registry: None,
        }
    }

    /// ChildProcessRegistry を設定する builder メソッド。
    pub fn with_registry(mut self, registry: ChildProcessRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Attempt to reserve a session slot before performing async I/O.
    ///
    /// Returns `true` and increments the pending counter when the combined
    /// count of active sessions and pending reservations is below `max`.
    /// Returns `false` (without mutating state) when the limit is already
    /// reached.
    ///
    /// The caller **must** subsequently call either [`register`] (to convert
    /// the reservation into a live session) or [`release_reservation`] (to
    /// free the slot on any error path that prevents registration).
    pub fn try_reserve(&mut self, max: usize) -> bool {
        if self.sessions.len() + self.pending >= max {
            return false;
        }
        self.pending += 1;
        true
    }

    /// Release a reservation obtained via [`try_reserve`].
    ///
    /// Call this on every error path that prevents [`register`] from being
    /// reached after a successful `try_reserve`.
    pub fn release_reservation(&mut self) {
        self.pending = self.pending.saturating_sub(1);
    }

    /// Total active sessions plus pending reservations.
    ///
    /// Useful for assertions and observability; the session limit is enforced
    /// against this value by [`try_reserve`].
    pub fn count_effective(&self) -> usize {
        self.sessions.len() + self.pending
    }

    pub fn register(&mut self, issue_id: i64, state: State, mut child: Child, run_id: i64) {
        // Consume the reservation that was taken via try_reserve (if any).
        self.pending = self.pending.saturating_sub(1);

        if let Some(mut old_entry) = self.sessions.remove(&issue_id) {
            let old_pid = old_entry.child.id();
            let _ = old_entry.child.kill();
            let wait_result = old_entry.child.wait();
            if wait_result.is_ok()
                && let Some(ref reg) = self.registry
            {
                reg.unregister(old_pid);
            }
        }

        let pid = child.id();

        if let Err(e) = std::fs::create_dir_all(&self.log_dir) {
            tracing::warn!(
                path = %self.log_dir.display(),
                error = %e,
                "failed to create process log directory"
            );
        }

        let (stdout_path, stderr_path) = if run_id > 0 {
            (
                self.log_dir.join(format!("run-{run_id}-stdout.log")),
                self.log_dir.join(format!("run-{run_id}-stderr.log")),
            )
        } else {
            tracing::warn!(
                issue_id,
                run_id,
                "non-positive run_id; using issue_id-based log paths to avoid collisions"
            );
            (
                self.log_dir.join(format!("issue-{issue_id}-stdout.log")),
                self.log_dir.join(format!("issue-{issue_id}-stderr.log")),
            )
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if stdout.is_none() {
            tracing::warn!(issue_id, "child stdout is None (Stdio::piped not set?)");
        }
        if stderr.is_none() {
            tracing::warn!(issue_id, "child stderr is None (Stdio::piped not set?)");
        }

        let stdout_reader_handle = start_reader_thread(stdout, stdout_path.clone());
        let stderr_reader_handle = start_reader_thread(stderr, stderr_path.clone());

        tracing::debug!(
            issue_id,
            stdout_path = %stdout_path.display(),
            stderr_path = %stderr_path.display(),
            "started reader threads for process log"
        );

        self.sessions.insert(
            issue_id,
            SessionEntry {
                child,
                started_at: Instant::now(),
                registered_state: state,
                stdout_path,
                stderr_path,
                stdout_reader_handle,
                stderr_reader_handle,
                log_id: 0,
                run_id,
            },
        );

        if let Some(ref reg) = self.registry {
            reg.register(pid);
        }
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
                let pid = entry.child.id();

                #[expect(
                    clippy::expect_used,
                    reason = "wait after kill is practically unreachable"
                )]
                let exit_status = entry.child.try_wait().ok().flatten().unwrap_or_else(|| {
                    let _ = entry.child.kill();
                    entry.child.wait().expect("wait after kill")
                });

                // Wait for reader threads to complete writing before returning paths
                if let Some(h) = entry.stdout_reader_handle.take() {
                    let _ = h.join();
                }
                if let Some(h) = entry.stderr_reader_handle.take() {
                    let _ = h.join();
                }

                if let Some(ref reg) = self.registry {
                    reg.unregister(pid);
                }

                if let Some(ref reg) = self.registry {
                    reg.unregister(pid);
                }

                results.push(ExitedSession {
                    issue_id,
                    exit_status,
                    stdout_path: entry.stdout_path,
                    stderr_path: entry.stderr_path,
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
            let pid = entry.child.id();
            entry.child.kill()?;
            if let Some(ref reg) = self.registry {
                reg.unregister(pid);
            }
        }
        Ok(())
    }

    pub fn kill_all(&mut self) {
        let pids: Vec<u32> = self.sessions.values().map(|e| e.child.id()).collect();
        for entry in self.sessions.values_mut() {
            let _ = entry.child.kill();
        }
        if let Some(ref reg) = self.registry {
            for pid in pids {
                reg.unregister(pid);
            }
        }
    }

    pub fn update_log_id(&mut self, issue_id: i64, log_id: i64) {
        if let Some(entry) = self.sessions.get_mut(&issue_id) {
            entry.log_id = log_id;
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

impl SessionManager {
    pub fn with_log_dir(log_dir: PathBuf) -> Self {
        Self {
            sessions: HashMap::new(),
            pending: 0,
            log_dir,
            registry: None,
        }
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
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(10);
        mgr.register(1, State::DesignRunning, child, 1001);

        assert!(mgr.is_running(1));
        assert!(!mgr.is_running(2));

        mgr.kill_all();
    }

    #[test]
    fn collect_exited_returns_finished_process() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_echo();
        mgr.register(1, State::DesignRunning, child, 1002);

        // Wait for echo to finish
        std::thread::sleep(Duration::from_millis(200));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].issue_id, 1);
        assert!(exited[0].exit_status.success());
        let stdout =
            std::fs::read_to_string(&exited[0].stdout_path).expect("stdout log should exist");
        assert!(stdout.contains("hello from stdout"));
        assert!(!mgr.is_running(1));
    }

    #[test]
    fn collect_exited_skips_running_process() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(10);
        mgr.register(1, State::DesignRunning, child, 1003);

        let exited = mgr.collect_exited();
        assert!(exited.is_empty());
        assert!(mgr.is_running(1));

        mgr.kill_all();
    }

    #[test]
    fn kill_stops_process() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child, 1004);

        mgr.kill(1).expect("kill should work");

        // After kill, collect_exited should return it
        std::thread::sleep(Duration::from_millis(100));
        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert!(!exited[0].exit_status.success());
    }

    #[test]
    fn count_reflects_registered_and_exited() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        assert_eq!(mgr.count(), 0);

        let child1 = spawn_echo();
        mgr.register(1, State::DesignRunning, child1, 1005);
        assert_eq!(mgr.count(), 1);

        let child2 = spawn_sleep(10);
        mgr.register(2, State::ImplementationRunning, child2, 1006);
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
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_echo();
        mgr.register(1, State::DesignRunning, child, 1007);
        mgr.update_log_id(1, 42);

        std::thread::sleep(Duration::from_millis(200));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].log_id, 42);
    }

    #[test]
    fn find_stalled_detects_old_processes() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child, 1008);

        // With a very short timeout, everything is stalled
        let stalled = mgr.find_stalled(Duration::from_nanos(1));
        assert_eq!(stalled, vec![1]);

        // With a long timeout, nothing is stalled
        let stalled = mgr.find_stalled(Duration::from_secs(3600));
        assert!(stalled.is_empty());

        mgr.kill_all();
    }

    // --- try_reserve / release_reservation tests ---

    /// T-3.SM.R1: try_reserve returns false when the limit is already reached
    /// by active sessions.
    #[test]
    fn try_reserve_returns_false_when_sessions_at_limit() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child, 1009);
        assert_eq!(mgr.count(), 1);

        assert!(
            !mgr.try_reserve(1),
            "limit already reached by active session"
        );
        assert_eq!(mgr.count_effective(), 1);

        mgr.kill_all();
    }

    /// T-3.SM.R2: pending reservations count toward the limit so a second
    /// try_reserve correctly sees the slot as taken.
    #[test]
    fn pending_counts_toward_limit() {
        let mut mgr = SessionManager::new();
        assert!(mgr.try_reserve(2), "first reservation should succeed");
        assert!(mgr.try_reserve(2), "second reservation should succeed");
        assert!(
            !mgr.try_reserve(2),
            "limit (2) reached by two pending slots"
        );
        assert_eq!(mgr.count_effective(), 2);

        // Clean up.
        mgr.release_reservation();
        mgr.release_reservation();
        assert_eq!(mgr.count_effective(), 0);
    }

    /// T-3.SM.R3: release_reservation frees the slot so a subsequent
    /// try_reserve can succeed again.
    #[test]
    fn release_reservation_frees_slot() {
        let mut mgr = SessionManager::new();
        assert!(mgr.try_reserve(1));
        assert!(!mgr.try_reserve(1), "at limit after reservation");

        mgr.release_reservation();
        assert!(mgr.try_reserve(1), "slot freed after release");

        mgr.release_reservation();
        assert_eq!(mgr.count_effective(), 0);
    }

    /// T-3.SM.R4: register() consumes the pending reservation so that a new
    /// try_reserve can see the updated count correctly.
    #[test]
    fn register_consumes_reservation() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        assert!(mgr.try_reserve(1));
        assert_eq!(mgr.count_effective(), 1);

        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child, 1010);

        // pending = 0, sessions.len() = 1 → count_effective = 1.
        // A new try_reserve(1) should fail (limit is 1).
        assert!(
            !mgr.try_reserve(1),
            "session is registered, limit still reached"
        );
        // But try_reserve(2) should succeed (1 active < 2).
        assert!(mgr.try_reserve(2));

        mgr.release_reservation();
        mgr.kill_all();
    }

    /// T-3.SM.R5: N concurrent try_reserve calls where N > max_limit result in
    /// at most max_limit successes (sequential stress test).
    #[test]
    fn session_limit_not_exceeded_under_sequential_stress() {
        let mut mgr = SessionManager::new();
        let max = 3usize;
        let attempts = 8usize;

        let mut successes = 0usize;
        for _ in 0..attempts {
            if mgr.try_reserve(max) {
                successes += 1;
            }
        }

        assert_eq!(
            successes, max,
            "exactly max_limit reservations should succeed"
        );
        assert_eq!(mgr.count_effective(), max);

        // Release all.
        for _ in 0..successes {
            mgr.release_reservation();
        }
        assert_eq!(mgr.count_effective(), 0);
    }

    #[test]
    fn register_kills_old_process_on_duplicate_issue_id() {
        use nix::sys::signal;
        use nix::unistd::Pid;

        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());

        // 最初のsleepプロセスを登録
        let old_child = spawn_sleep(60);
        let old_pid = old_child.id();
        mgr.register(1, State::DesignRunning, old_child, 1011);

        // 同一 issue_id に別プロセスを再登録すると旧プロセスがkillされ、wait()で回収される
        let new_child = spawn_sleep(60);
        mgr.register(1, State::ImplementationRunning, new_child, 1012);

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

    /// T-4.1: ログファイル作成失敗でも子プロセスが正常完了することを確認する
    #[test]
    fn log_write_failure_does_not_kill_child() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        // ログディレクトリのパスにファイルを作成して、create_dir_all + File::create を失敗させる
        let log_dir = tmpdir.path().join("process-runs");
        std::fs::write(&log_dir, "not a directory").expect("create blocking file");

        let mut mgr = SessionManager::with_log_dir(log_dir);
        let child = spawn_echo();
        mgr.register(1, State::DesignRunning, child, 9999);

        // Wait for echo to complete (pipe is drained so process is not blocked)
        std::thread::sleep(Duration::from_millis(300));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1, "child process should have completed");
        assert!(
            exited[0].exit_status.success(),
            "child process should have succeeded despite log write failure"
        );
    }

    /// T-5.3: プロセス実行後にログファイルへ書き込まれていることを確認する（統合テスト）
    #[test]
    fn log_is_written_after_process_completion() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());

        let child = Command::new("sh")
            .arg("-c")
            .arg("echo line1; echo line2; echo line3")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");

        mgr.register(1, State::DesignRunning, child, 8888);

        // Wait for process to complete
        std::thread::sleep(Duration::from_millis(300));

        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);
        assert!(exited[0].exit_status.success());

        let content =
            std::fs::read_to_string(&exited[0].stdout_path).expect("stdout log should exist");
        assert!(
            content.contains("line1"),
            "log should contain line1; content: {content:?}"
        );
        assert!(
            content.contains("line2"),
            "log should contain line2; content: {content:?}"
        );
        assert!(
            content.contains("line3"),
            "log should contain line3; content: {content:?}"
        );
    }

    // --- ChildProcessRegistry unit tests ---

    #[test]
    fn registry_register_and_unregister() {
        let reg = ChildProcessRegistry::new();
        reg.register(100);
        reg.register(200);
        assert!(reg.contains_pid(100));
        assert!(reg.contains_pid(200));
        reg.unregister(100);
        assert!(!reg.contains_pid(100));
        assert!(reg.contains_pid(200));
        reg.unregister(200);
        assert!(reg.pids_is_empty());
    }

    #[test]
    fn registry_unregister_nonexistent_is_safe() {
        let reg = ChildProcessRegistry::new();
        reg.unregister(9999); // should not panic
    }

    #[test]
    fn registry_shutdown_sync_empty_is_safe() {
        let reg = ChildProcessRegistry::new();
        reg.shutdown_sync(Duration::from_millis(100)); // should not panic
    }

    // --- SessionManager + ChildProcessRegistry integration tests ---

    #[test]
    fn registry_pid_added_on_register() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let reg = ChildProcessRegistry::new();
        let mut mgr =
            SessionManager::with_log_dir(tmpdir.path().to_path_buf()).with_registry(reg.clone());
        let child = spawn_sleep(60);
        let pid = child.id();
        mgr.register(1, State::DesignRunning, child, 0);

        assert!(
            reg.contains_pid(pid),
            "PID should be in registry after register"
        );

        mgr.kill_all();
    }

    #[test]
    fn registry_pid_removed_on_collect_exited() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let reg = ChildProcessRegistry::new();
        let mut mgr =
            SessionManager::with_log_dir(tmpdir.path().to_path_buf()).with_registry(reg.clone());
        let child = spawn_echo();
        let pid = child.id();
        mgr.register(1, State::DesignRunning, child, 0);

        std::thread::sleep(Duration::from_millis(200));
        let exited = mgr.collect_exited();
        assert_eq!(exited.len(), 1);

        assert!(
            !reg.contains_pid(pid),
            "PID should be removed from registry after collect_exited"
        );
    }

    #[test]
    fn registry_pid_removed_on_kill_all() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let reg = ChildProcessRegistry::new();
        let mut mgr =
            SessionManager::with_log_dir(tmpdir.path().to_path_buf()).with_registry(reg.clone());
        let child1 = spawn_sleep(60);
        let pid1 = child1.id();
        let child2 = spawn_sleep(60);
        let pid2 = child2.id();
        mgr.register(1, State::DesignRunning, child1, 0);
        mgr.register(2, State::ImplementationRunning, child2, 0);

        mgr.kill_all();

        assert!(
            !reg.contains_pid(pid1),
            "pid1 should be removed after kill_all"
        );
        assert!(
            !reg.contains_pid(pid2),
            "pid2 should be removed after kill_all"
        );
    }

    #[test]
    fn no_registry_existing_behavior_unchanged() {
        let tmpdir = tempfile::tempdir().expect("temp dir");
        let mut mgr = SessionManager::with_log_dir(tmpdir.path().to_path_buf());
        let child = spawn_sleep(60);
        mgr.register(1, State::DesignRunning, child, 0);
        assert!(mgr.is_running(1));
        mgr.kill_all();
    }

    #[test]
    fn registry_shutdown_sync_terminates_sleep_process() {
        let reg = ChildProcessRegistry::new();
        let mut child = spawn_sleep(60);
        let pid = child.id();
        reg.register(pid);

        // shutdown_sync sends SIGTERM; use a short timeout for fast test execution
        reg.shutdown_sync(Duration::from_millis(500));

        // child.try_wait() properly handles zombies (processes that exited but weren't reaped)
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {
                    assert!(
                        Instant::now() < deadline,
                        "process pid={pid} should have exited after shutdown_sync"
                    );
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("try_wait error: {e}"),
            }
        };
        assert!(
            !status.success(),
            "sleep should have been killed by signal, not exited normally"
        );
    }
}
