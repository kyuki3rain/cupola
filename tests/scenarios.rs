#![allow(clippy::expect_used)]
// Integration scenario tests (T-7.IT.*)
//
// These tests verify end-to-end scenarios using the `decide()` pure function
// against in-memory SQLite and mock GitHub adapters. They mirror the シナリオ例
// section in docs/architecture/.
//
// The tests use the domain `decide()` function directly since the full 5-phase
// polling loop requires a running tokio runtime with real process spawning.
// The scenario tests focus on verifying state machine correctness for each
// documented scenario.

use cupola::adapter::outbound::sqlite_connection::SqliteConnection;
use cupola::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use cupola::adapter::outbound::sqlite_process_run_repository::SqliteProcessRunRepository;
use cupola::application::port::issue_repository::IssueRepository;
use cupola::application::port::process_run_repository::ProcessRunRepository;
use cupola::domain::config::Config;
use cupola::domain::decide::decide;
use cupola::domain::effect::Effect;
use cupola::domain::issue::Issue;
use cupola::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
use cupola::domain::state::State;
use cupola::domain::task_weight::TaskWeight;
use cupola::domain::world_snapshot::{
    CiStatus, GithubIssueSnapshot, PrSnapshot, PrState, ProcessSnapshot, ProcessesSnapshot,
    WorldSnapshot,
};

fn setup_db() -> (SqliteIssueRepository, SqliteProcessRunRepository) {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    (
        SqliteIssueRepository::new(db.clone()),
        SqliteProcessRunRepository::new(db),
    )
}

fn default_config() -> Config {
    Config::default_with_repo("owner".into(), "repo".into(), "main".into())
}

fn new_issue(id: i64, number: u64, state: State) -> Issue {
    Issue {
        id,
        github_issue_number: number,
        state,
        feature_name: format!("issue-{number}"),
        weight: TaskWeight::Medium,
        worktree_path: None,
        ci_fix_count: 0,
        close_finished: false,
        consecutive_failures_epoch: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn open_issue_snap() -> GithubIssueSnapshot {
    GithubIssueSnapshot::Open {
        has_ready_label: false,
        ready_label_trusted: false,
        weight: Some(TaskWeight::Medium),
    }
}

fn closed_issue_snap() -> GithubIssueSnapshot {
    GithubIssueSnapshot::Closed
}

fn ready_issue_snap(trusted: bool) -> GithubIssueSnapshot {
    GithubIssueSnapshot::Open {
        has_ready_label: true,
        ready_label_trusted: trusted,
        weight: Some(TaskWeight::Medium),
    }
}

fn open_pr_snap() -> PrSnapshot {
    PrSnapshot {
        state: PrState::Open,
        has_review_comments: false,
        ci_status: CiStatus::Unknown,
        has_conflict: false,
    }
}

fn merged_pr_snap() -> PrSnapshot {
    PrSnapshot {
        state: PrState::Merged,
        has_review_comments: false,
        ci_status: CiStatus::Unknown,
        has_conflict: false,
    }
}

fn succeeded_process(idx: u32) -> ProcessSnapshot {
    ProcessSnapshot {
        state: ProcessRunState::Succeeded,
        index: idx,
        run_id: 0,
        consecutive_failures: 0,
    }
}

fn failed_process(idx: u32, consecutive: u32) -> ProcessSnapshot {
    ProcessSnapshot {
        state: ProcessRunState::Failed,
        index: idx,
        run_id: 0,
        consecutive_failures: consecutive,
    }
}

fn no_processes() -> ProcessesSnapshot {
    ProcessesSnapshot {
        init: None,
        design: None,
        design_fix: None,
        impl_: None,
        impl_fix: None,
    }
}

/// T-7.IT.normal: Idle → InitializeRunning → DesignRunning → DesignReviewWaiting →
/// ImplementationRunning → ImplementationReviewWaiting → Completed (full happy path)
#[test]
fn t_7_it_normal_full_happy_path() {
    let config = default_config();

    // Step 1: Idle + ready label (trusted) → InitializeRunning
    let issue = new_issue(1, 42, State::Idle);
    let snap = WorldSnapshot {
        github_issue: ready_issue_snap(true),
        design_pr: None,
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::InitializeRunning,
        "Idle→InitializeRunning"
    );
    // Note: SpawnInit is emitted in InitializeRunning state, not from the Idle→InitializeRunning transition

    // Step 1b: InitializeRunning with no init process → SpawnInit emitted
    let issue = new_issue(1, 42, State::InitializeRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: None,
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::InitializeRunning,
        "should stay InitializeRunning until init runs"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::SpawnInit)),
        "should emit SpawnInit when no init process exists"
    );

    // Step 2: InitializeRunning + init succeeded + no PR → DesignRunning
    let issue = new_issue(1, 42, State::InitializeRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: None,
        impl_pr: None,
        processes: ProcessesSnapshot {
            init: Some(succeeded_process(0)),
            ..no_processes()
        },
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::DesignRunning,
        "InitializeRunning→DesignRunning"
    );

    // Step 3: DesignRunning + design succeeded + design_pr open → DesignReviewWaiting
    let issue = new_issue(1, 42, State::DesignRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(open_pr_snap()),
        impl_pr: None,
        processes: ProcessesSnapshot {
            design: Some(succeeded_process(0)),
            ..no_processes()
        },
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::DesignReviewWaiting,
        "DesignRunning→DesignReviewWaiting"
    );

    // Step 4: DesignReviewWaiting + design_pr merged → ImplementationRunning
    let issue = new_issue(1, 42, State::DesignReviewWaiting);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(merged_pr_snap()),
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::ImplementationRunning,
        "DesignReviewWaiting→ImplementationRunning"
    );

    // Step 5: ImplementationRunning + impl succeeded + impl_pr open → ImplementationReviewWaiting
    let issue = new_issue(1, 42, State::ImplementationRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(merged_pr_snap()),
        impl_pr: Some(open_pr_snap()),
        processes: ProcessesSnapshot {
            impl_: Some(succeeded_process(0)),
            ..no_processes()
        },
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::ImplementationReviewWaiting,
        "ImplementationRunning→ImplementationReviewWaiting"
    );

    // Step 6: ImplementationReviewWaiting + impl_pr merged → Completed
    let issue = new_issue(1, 42, State::ImplementationReviewWaiting);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: None,
        impl_pr: Some(merged_pr_snap()),
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::Completed,
        "ImplementationReviewWaiting→Completed"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PostCompletedComment)),
        "should emit PostCompletedComment"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::CloseIssue)),
        "should emit CloseIssue"
    );
}

/// T-7.IT.recovery: InitializeRunning + design_pr already open → DesignReviewWaiting (smart routing)
#[test]
fn t_7_it_recovery_smart_route_to_design_review_waiting() {
    let config = default_config();

    // InitializeRunning + init succeeded + design_pr open (already exists from prev run)
    let issue = new_issue(1, 42, State::InitializeRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(open_pr_snap()),
        impl_pr: None,
        processes: ProcessesSnapshot {
            init: Some(succeeded_process(0)),
            ..no_processes()
        },
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::DesignReviewWaiting,
        "should smart-route to DesignReviewWaiting when design PR already open"
    );
}

/// T-7.IT.fixing_merge: DesignFixing while reviewer merges → ImplementationRunning
#[test]
fn t_7_it_fixing_merge_design_fixing_pr_merged() {
    let config = default_config();

    let issue = new_issue(1, 42, State::DesignFixing);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(merged_pr_snap()),
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::ImplementationRunning,
        "DesignFixing + PR merged → ImplementationRunning"
    );
}

/// T-7.IT.cancel_close: Issue closed mid-flow → Cancelled + PostCancelComment + CloseIssue
#[test]
fn t_7_it_cancel_close_issue_closed_from_design_running() {
    let config = default_config();

    let issue = new_issue(1, 42, State::DesignRunning);
    let snap = WorldSnapshot {
        github_issue: closed_issue_snap(),
        design_pr: None,
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::Cancelled,
        "issue closed → Cancelled"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PostCancelComment)),
        "should emit PostCancelComment"
    );
}

/// T-7.IT.cancel_retry: Process keeps failing → Cancelled + PostRetryExhaustedComment
#[test]
fn t_7_it_cancel_retry_process_failure_exhausted() {
    let config = default_config(); // max_retries = 3

    let issue = new_issue(1, 42, State::DesignRunning);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: None,
        impl_pr: None,
        processes: ProcessesSnapshot {
            design: Some(failed_process(3, 3)), // consecutive_failures >= max_retries
            ..no_processes()
        },
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::Cancelled,
        "exhausted retries → Cancelled"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PostRetryExhaustedComment { .. })),
        "should emit PostRetryExhaustedComment"
    );
}

/// T-7.IT.reopen: Cancelled + close_finished + Issue reopened → Idle
#[test]
fn t_7_it_reopen_cancelled_reopened_goes_idle() {
    let config = default_config();

    let mut issue = new_issue(1, 42, State::Cancelled);
    issue.close_finished = true;

    let snap = WorldSnapshot {
        github_issue: GithubIssueSnapshot::Open {
            has_ready_label: false,
            ready_label_trusted: false,
            weight: Some(TaskWeight::Medium),
        },
        design_pr: None,
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::Idle,
        "Cancelled + reopened → Idle"
    );
}

/// T-7.IT.pr_closed: PR closed (not merged) in DesignReviewWaiting → DesignRunning
#[test]
fn t_7_it_pr_closed_design_review_waiting_goes_design_running() {
    let config = default_config();

    let issue = new_issue(1, 42, State::DesignReviewWaiting);
    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(PrSnapshot {
            state: PrState::Closed,
            has_review_comments: false,
            ci_status: CiStatus::Unknown,
            has_conflict: false,
        }),
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false,
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::DesignRunning,
        "PR closed in DesignReviewWaiting → DesignRunning"
    );
}

/// T-7.IT.ci_limit: ReviewWaiting + CI exhausted → PostCiFixLimitComment, no further fixing
#[test]
fn t_7_it_ci_limit_posts_limit_comment_and_stays() {
    let config = default_config(); // max_ci_fix_cycles = 3

    let mut issue = new_issue(1, 42, State::DesignReviewWaiting);
    issue.ci_fix_count = 3; // == max_ci_fix_cycles

    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(PrSnapshot {
            state: PrState::Open,
            has_review_comments: false,
            ci_status: CiStatus::Failure,
            has_conflict: false,
        }),
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: true, // ci_fix_count >= max
    };
    let decision = decide(&issue, &snap, &config);
    // Should stay in DesignReviewWaiting and post limit comment
    assert_eq!(
        decision.next_state,
        State::DesignReviewWaiting,
        "should stay in DesignReviewWaiting when CI fix limit reached"
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PostCiFixLimitComment)),
        "should emit PostCiFixLimitComment"
    );
}

/// T-7.IT.review_resets_ci: CI count maxed, then trusted reviewer adds comment
/// → DesignFixing + ci_fix_count reset
#[test]
fn t_7_it_review_resets_ci_trusted_review_resets_count() {
    let config = default_config();

    let mut issue = new_issue(1, 42, State::DesignReviewWaiting);
    issue.ci_fix_count = 3; // was at max

    let snap = WorldSnapshot {
        github_issue: open_issue_snap(),
        design_pr: Some(PrSnapshot {
            state: PrState::Open,
            has_review_comments: true, // trusted reviewer added comment
            ci_status: CiStatus::Failure,
            has_conflict: false,
        }),
        impl_pr: None,
        processes: no_processes(),
        ci_fix_exhausted: false, // review resets the exhaustion check (ci_fix_count was reset)
    };
    let decision = decide(&issue, &snap, &config);
    assert_eq!(
        decision.next_state,
        State::DesignFixing,
        "review comment → DesignFixing (even if CI was failing)"
    );
    // ci_fix_count should be reset to 0
    assert_eq!(
        decision.metadata_updates.ci_fix_count,
        Some(0),
        "ci_fix_count should be reset when reviewer comment takes over"
    );
}

/// T-7.IT.orphan_recovery: Startup with running process_runs → all marked failed
#[tokio::test]
async fn t_7_it_orphan_recovery_startup_marks_all_failed() {
    use cupola::application::start_use_case::{ProcessAlivePort, recover_orphans};

    let (issue_repo, process_repo) = setup_db();
    let issue_id = issue_repo
        .save(&new_issue(0, 42, State::DesignRunning))
        .await
        .expect("save issue");

    // Insert running process run with PID
    let run = ProcessRun::new_running(issue_id, ProcessRunType::Design, 0, vec![]);
    let run_id = process_repo.save(&run).await.expect("save run");
    process_repo
        .update_pid(run_id, 12345)
        .await
        .expect("update pid");

    // Insert running process run without PID
    let run2 = ProcessRun::new_running(issue_id, ProcessRunType::Impl, 0, vec![]);
    process_repo.save(&run2).await.expect("save run2");

    struct AlwaysDeadPort;
    impl ProcessAlivePort for AlwaysDeadPort {
        fn is_alive(&self, _pid: u32) -> bool {
            false // all processes are dead
        }
        fn kill(&self, _pid: u32) {}
    }

    let result = recover_orphans(&process_repo, &AlwaysDeadPort)
        .await
        .expect("recover_orphans");

    assert_eq!(
        result.recovered_count, 2,
        "both running records should be recovered"
    );

    // All runs should now be failed
    let runs = process_repo.find_by_issue(issue_id).await.expect("find");
    for run in &runs {
        assert_eq!(
            run.state,
            ProcessRunState::Failed,
            "all runs should be failed after orphan recovery"
        );
        assert_eq!(
            run.error_message.as_deref(),
            Some("orphaned"),
            "error_message should be 'orphaned'"
        );
    }
}
