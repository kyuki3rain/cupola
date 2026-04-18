/// Start-use-case: 起動時の孤児プロセス回収 (orphan recovery)
///
/// cupola start を実行した際、前回の異常終了でプロセスが残っている可能性がある。
/// process_runs.state='running' のレコードを全て検索し、
/// - PID が有効な場合は SIGKILL を送信
/// - いずれの場合もレコードを state='failed', error_message='orphaned' に更新する。
use anyhow::Result;

use crate::application::port::process_run_repository::ProcessRunRepository;

/// orphan recovery の実行結果
#[derive(Debug, Default)]
pub struct OrphanRecoveryResult {
    /// 回収した孤児プロセスの数
    pub recovered_count: usize,
    /// SIGKILL を送信した PID のリスト
    pub killed_pids: Vec<u32>,
}

/// プロセスが生きているかどうかを確認するポート
pub trait ProcessAlivePort: Send + Sync {
    fn is_alive(&self, pid: u32) -> bool;
    fn kill(&self, pid: u32);
}

/// 起動時の孤児プロセス回収を実行する。
///
/// process_runs.state='running' のレコードを全て探し、
/// - PID が Some かつプロセスが生きていれば SIGKILL
/// - state を 'failed', error_message='orphaned' に更新
pub async fn recover_orphans<P, A>(process_repo: &P, alive_port: &A) -> Result<OrphanRecoveryResult>
where
    P: ProcessRunRepository,
    A: ProcessAlivePort,
{
    let running = process_repo.find_all_running().await?;
    let mut result = OrphanRecoveryResult::default();

    for run in &running {
        match run.pid {
            Some(pid) if alive_port.is_alive(pid) => {
                alive_port.kill(pid);
                result.killed_pids.push(pid);
            }
            None => {
                // PID が NULL — プロセスは spawn されたが update_pid が呼ばれる前にデーモンがクラッシュした可能性がある。
                // 孤児プロセスが残っている可能性があるが、PID が不明なため kill できない。
                tracing::warn!(
                    process_run_id = run.id,
                    issue_id = run.issue_id,
                    run_type = %run.type_,
                    "orphan process has pid=NULL; daemon may have crashed before update_pid — cannot kill, marking failed"
                );
            }
            Some(_) => {
                // PID はあるが既に終了済み — kill 不要
            }
        }

        // レコードを failed に更新
        process_repo
            .mark_failed(run.id, Some("orphaned".to_string()))
            .await?;
        result.recovered_count += 1;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use crate::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
    use crate::adapter::outbound::sqlite_process_run_repository::SqliteProcessRunRepository;
    use crate::application::port::issue_repository::IssueRepository;
    use crate::domain::issue::Issue;
    use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
    use crate::domain::state::State;

    fn setup() -> (SqliteProcessRunRepository, SqliteIssueRepository) {
        let db = SqliteConnection::open_in_memory().expect("open in-memory");
        db.init_schema().expect("init schema");
        (
            SqliteProcessRunRepository::new(db.clone()),
            SqliteIssueRepository::new(db),
        )
    }

    fn new_issue(n: u64) -> Issue {
        Issue {
            id: 0,
            github_issue_number: n,
            state: State::DesignRunning,
            worktree_path: None,
            ci_fix_count: 0,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            feature_name: format!("issue-{n}"),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Mock プロセス生存確認ポート
    struct MockAlivePort {
        alive_pids: Vec<u32>,
        killed: Arc<Mutex<Vec<u32>>>,
    }

    impl MockAlivePort {
        fn new(alive_pids: Vec<u32>) -> (Self, Arc<Mutex<Vec<u32>>>) {
            let killed = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    alive_pids,
                    killed: killed.clone(),
                },
                killed,
            )
        }
    }

    impl ProcessAlivePort for MockAlivePort {
        fn is_alive(&self, pid: u32) -> bool {
            self.alive_pids.contains(&pid)
        }

        fn kill(&self, pid: u32) {
            self.killed.lock().unwrap().push(pid);
        }
    }

    /// T-5.O.1: 起動時に state='running' の全レコードが SIGKILL + failed に更新される
    #[tokio::test]
    async fn t_5_o_1_all_running_records_are_killed_and_marked_failed() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(1)).await.expect("save issue");

        // 2 つの running レコードを作成 (PID 付き)
        let run1 = ProcessRun::new_running(issue_id, ProcessRunType::Design, 0, vec![]);
        let id1 = repo.save(&run1).await.expect("save run1");
        repo.update_pid(id1, 1001).await.expect("update pid");

        let run2 = ProcessRun::new_running(issue_id, ProcessRunType::Impl, 0, vec![]);
        let id2 = repo.save(&run2).await.expect("save run2");
        repo.update_pid(id2, 1002).await.expect("update pid");

        let (alive_port, killed) = MockAlivePort::new(vec![1001, 1002]);
        let result = recover_orphans(&repo, &alive_port)
            .await
            .expect("recover_orphans");

        assert_eq!(result.recovered_count, 2, "both runs should be recovered");
        assert_eq!(
            killed.lock().unwrap().len(),
            2,
            "both PIDs should be killed"
        );

        // DB 上でも failed になっていることを確認
        let runs = repo.find_by_issue(issue_id).await.expect("find");
        for run in &runs {
            assert_eq!(run.state, ProcessRunState::Failed, "state should be failed");
            assert_eq!(
                run.error_message.as_deref(),
                Some("orphaned"),
                "error_message should be 'orphaned'"
            );
        }
    }

    /// T-5.O.2: PID=null の running レコードも failed に更新される（kill なし）
    #[tokio::test]
    async fn t_5_o_2_null_pid_running_rows_are_marked_failed_without_kill() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(2)).await.expect("save issue");

        // PID なし running レコード
        let run = ProcessRun::new_running(issue_id, ProcessRunType::Init, 0, vec![]);
        repo.save(&run).await.expect("save run");
        // pid を更新しない → None のまま

        let (alive_port, killed) = MockAlivePort::new(vec![]);
        let result = recover_orphans(&repo, &alive_port)
            .await
            .expect("recover_orphans");

        assert_eq!(result.recovered_count, 1);
        assert!(
            killed.lock().unwrap().is_empty(),
            "no kills for null-PID rows"
        );

        let runs = repo.find_by_issue(issue_id).await.expect("find");
        assert_eq!(runs[0].state, ProcessRunState::Failed);
        assert_eq!(runs[0].error_message.as_deref(), Some("orphaned"));
    }

    /// T-5.O.3: 生きている PID は exactly once kill される
    #[tokio::test]
    async fn t_5_o_3_live_process_killed_exactly_once() {
        let (repo, issue_repo) = setup();
        let issue_id = issue_repo.save(&new_issue(3)).await.expect("save issue");

        let run = ProcessRun::new_running(issue_id, ProcessRunType::Design, 0, vec![]);
        let id = repo.save(&run).await.expect("save run");
        repo.update_pid(id, 9999).await.expect("update pid");

        let (alive_port, killed) = MockAlivePort::new(vec![9999]);
        recover_orphans(&repo, &alive_port)
            .await
            .expect("recover_orphans");

        let killed_list = killed.lock().unwrap();
        assert_eq!(killed_list.len(), 1, "should be killed exactly once");
        assert_eq!(killed_list[0], 9999);
    }
}
