#!/usr/bin/env bash
# scripts/e2e/run.sh — Cupola E2E test runner
set -euo pipefail
IFS=$'\n\t'

# ---------------------------------------------------------------------------
# Locate script directory (robustly)
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------
usage() {
  cat >&2 <<'EOF'
usage: run.sh [OPTIONS] [SCENARIO_ID...]

Options:
  --keep-repo           do not delete repo on success
  --no-keep-dir         delete run directory on success
  --fail-fast           stop at first scenario failure
  --reuse-repo NAME     use existing repo (skip create+delete)
  --delete-repo NAME    delete specified repo and exit
  --owner OWNER         repo owner (default: gh current user)
  -h, --help            show this help

Scenario IDs:
  E-01   Happy path (Idle → Completed)
  E-02   Daemon lifecycle (start/stop/status)
  E-10   Doctor green

Examples:
  run.sh                   # Run smoke set: E-01, E-02, E-10
  run.sh E-02 E-10         # Run selected scenarios
  run.sh --keep-repo       # Preserve repo on success
  run.sh --delete-repo owner/cupola-e2e-20260408-153012-abc
EOF
}

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
KEEP_REPO=0
NO_KEEP_DIR=0
FAIL_FAST=0
REUSE_REPO=""
DELETE_REPO=""
OWNER=""
SCENARIO_IDS=()

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
  case "$1" in
    --keep-repo)
      KEEP_REPO=1; shift ;;
    --no-keep-dir)
      NO_KEEP_DIR=1; shift ;;
    --fail-fast)
      FAIL_FAST=1; shift ;;
    --reuse-repo)
      REUSE_REPO="$2"; shift 2 ;;
    --delete-repo)
      DELETE_REPO="$2"; shift 2 ;;
    --owner)
      OWNER="$2"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    E-01|E-02|E-10)
      SCENARIO_IDS+=("$1"); shift ;;
    -*)
      printf "Unknown option: %s\n" "$1" >&2
      usage
      exit 1
      ;;
    *)
      printf "Unknown scenario ID: %s\n" "$1" >&2
      printf "Available scenarios: E-01, E-02, E-10\n" >&2
      exit 1
      ;;
  esac
done

export KEEP_REPO NO_KEEP_DIR FAIL_FAST REUSE_REPO OWNER

# ---------------------------------------------------------------------------
# Change to repo root and load libraries
# ---------------------------------------------------------------------------
cd "$REPO_ROOT"

# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/prereqs.sh
. "$SCRIPT_DIR/lib/prereqs.sh"
# shellcheck source=lib/repo.sh
. "$SCRIPT_DIR/lib/repo.sh"
# shellcheck source=lib/cupola.sh
. "$SCRIPT_DIR/lib/cupola.sh"
# shellcheck source=lib/result.sh
. "$SCRIPT_DIR/lib/result.sh"

# ---------------------------------------------------------------------------
# Guard: must be run from cupola repo root
# ---------------------------------------------------------------------------
assert_repo_root

# ---------------------------------------------------------------------------
# --delete-repo short-circuit
# ---------------------------------------------------------------------------
if [ -n "$DELETE_REPO" ]; then
  log_info "Deleting repo: $DELETE_REPO"
  gh repo delete "$DELETE_REPO" --yes
  log_info "Deleted."
  exit 0
fi

# ---------------------------------------------------------------------------
# Default smoke set
# ---------------------------------------------------------------------------
if [ "${#SCENARIO_IDS[@]}" -eq 0 ]; then
  SCENARIO_IDS=(E-01 E-02 E-10)
fi

# ---------------------------------------------------------------------------
# Set up run directory
# ---------------------------------------------------------------------------
RUN_BASE="${HOME}/work/cupola-e2e-run"
mkdir -p "$RUN_BASE"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RUN_SUFFIX=$(random3)
RUN_DIR="${RUN_BASE}/${TIMESTAMP}-${RUN_SUFFIX}"
mkdir -p "$RUN_DIR"
export RUN_DIR

log_info "Run directory: $RUN_DIR"

# ---------------------------------------------------------------------------
# EXIT trap
# ---------------------------------------------------------------------------
EXIT_CODE=0
_on_exit() {
  teardown_repo "$EXIT_CODE" || true
  result_summary || true
}
trap '_on_exit' EXIT

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
ensure_prereqs

# ---------------------------------------------------------------------------
# Repo setup
# ---------------------------------------------------------------------------
if [ -n "$REUSE_REPO" ]; then
  # Parse owner/name
  REPO_OWNER="${REUSE_REPO%%/*}"
  REPO_NAME="${REUSE_REPO##*/}"
  REPO_FULL="$REUSE_REPO"
  OWNER="$REPO_OWNER"
  TARGET_DIR="$RUN_DIR/target"
  export REPO_OWNER REPO_NAME REPO_FULL OWNER TARGET_DIR
  log_info "Reusing existing repo: $REPO_FULL"
  # Clone if target dir doesn't exist
  if [ ! -d "$TARGET_DIR" ]; then
    gh repo clone "$REPO_FULL" "$TARGET_DIR"
  fi
else
  create_ephemeral_repo
fi

# ---------------------------------------------------------------------------
# Initialize cupola
# ---------------------------------------------------------------------------
init_cupola

# ---------------------------------------------------------------------------
# Source scenario files
# ---------------------------------------------------------------------------
# shellcheck source=scenarios/e01_happy_path.sh
. "$SCRIPT_DIR/scenarios/e01_happy_path.sh"
# shellcheck source=scenarios/e02_daemon_lifecycle.sh
. "$SCRIPT_DIR/scenarios/e02_daemon_lifecycle.sh"
# shellcheck source=scenarios/e10_doctor_green.sh
. "$SCRIPT_DIR/scenarios/e10_doctor_green.sh"

# ---------------------------------------------------------------------------
# Run scenarios
# ---------------------------------------------------------------------------
result_start
FAILED=0

run_scenario() {
  local id="$1"
  local fn
  case "$id" in
    E-01) fn="scenario_e01" ;;
    E-02) fn="scenario_e02" ;;
    E-10) fn="scenario_e10" ;;
    *)
      log_error "Unknown scenario: $id"
      return 1
      ;;
  esac

  log_section "Running scenario $id"
  local t_start t_end elapsed_secs
  t_start=$(date +%s)

  local rc=0
  "$fn" || rc=$?

  t_end=$(date +%s)
  elapsed_secs=$((t_end - t_start))

  if [ "$rc" -eq 0 ]; then
    log_info "Scenario $id: PASSED (${elapsed_secs}s)"
    result_append "$id" "pass" "$elapsed_secs" "ok"
  else
    log_error "Scenario $id: FAILED (${elapsed_secs}s, rc=$rc)"
    result_append "$id" "fail" "$elapsed_secs" "exit code $rc"
    FAILED=1
    if [ "$FAIL_FAST" -eq 1 ]; then
      log_error "--fail-fast: stopping after first failure."
      EXIT_CODE=1
      exit 1
    fi
  fi
}

for sid in "${SCENARIO_IDS[@]}"; do
  run_scenario "$sid"
done

# ---------------------------------------------------------------------------
# Final exit code
# ---------------------------------------------------------------------------
if [ "$FAILED" -ne 0 ]; then
  EXIT_CODE=1
fi

exit "$EXIT_CODE"
