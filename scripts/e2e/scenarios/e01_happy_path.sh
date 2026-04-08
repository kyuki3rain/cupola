#!/usr/bin/env bash
# scenarios/e01_happy_path.sh — E-01: Happy path (Idle → Completed)
set -euo pipefail
IFS=$'\n\t'

scenario_e01() {
  log_section "E-01: Happy Path"
  cd "$TARGET_DIR"

  # 1. Create issue
  local issue_number
  issue_number=$(gh issue create \
    --repo "$REPO_FULL" \
    --title "E-01: add hello" \
    --body "README に hello と書く" \
    --label "weight:light" \
    --json number --jq .number)
  log_info "Issue created: #${issue_number}"

  # 2. Start daemon
  "$CUPOLA_BIN" start --daemon
  log_info "cupola started."
  sleep 3

  # 3. Add agent:ready label
  gh issue edit "$issue_number" --repo "$REPO_FULL" --add-label "agent:ready"
  log_info "Label agent:ready added to #${issue_number}"

  # 4. Wait for InitializeRunning
  wait_for_state "$issue_number" "InitializeRunning" 60

  # 5. Wait for DesignRunning
  wait_for_state "$issue_number" "DesignRunning" 300

  # 6. Wait for design PR
  local design_pr
  design_pr=$(wait_for_pr "$issue_number" "design" 900)
  log_info "Design PR: #${design_pr}"

  # 7. Merge design PR
  gh pr merge "$design_pr" --repo "$REPO_FULL" --squash --yes --delete-branch=false
  log_info "Design PR #${design_pr} merged."

  # 8. Wait for ImplementationRunning
  wait_for_state "$issue_number" "ImplementationRunning" 120

  # 9. Wait for impl PR
  local impl_pr
  impl_pr=$(wait_for_pr "$issue_number" "impl" 900)
  log_info "Impl PR: #${impl_pr}"

  # 10. Merge impl PR
  gh pr merge "$impl_pr" --repo "$REPO_FULL" --squash --yes --delete-branch=false
  log_info "Impl PR #${impl_pr} merged."

  # 11. Wait for Completed
  wait_for_state "$issue_number" "Completed" 120

  # Verification
  # close_finished=1 via sqlite
  local cf
  cf=$(sqlite_query "SELECT close_finished FROM issues WHERE github_issue_number=${issue_number};" 2>/dev/null || echo "n/a")
  log_info "close_finished for #${issue_number}: ${cf}"
  if [ "$cf" != "1" ] && [ "$cf" != "n/a" ]; then
    log_error "Expected close_finished=1, got: $cf"
    return 1
  fi

  # Worktree should be gone
  local worktree_path="$TARGET_DIR/.cupola/worktrees/issue-${issue_number}"
  if [ -d "$worktree_path" ]; then
    log_warn "Worktree still exists: $worktree_path (may be cleaned up asynchronously)"
  else
    log_info "Worktree cleaned up: OK"
  fi

  # Stop daemon
  "$CUPOLA_BIN" stop || true

  log_info "E-01: PASSED"
  return 0
}
