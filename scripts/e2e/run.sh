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
usage: run.sh [OPTIONS]

Options:
  --keep-repo           do not delete repo on success
  --no-keep-dir         delete run directory on success
  --fail-fast           stop at first CP failure
  --from PHASE          start from the specified phase (e.g. --from phase_3)
  --only PHASE          run only the specified phase (e.g. --only phase_0)
  --reuse-repo NAME     use existing repo (skip create+delete)
  --delete-repo NAME    delete specified repo and exit
  --owner OWNER         repo owner (default: gh current user)
  -h, --help            show this help

Phases:
  phase_0   Pre-flight (doctor / init --upgrade / daemon lifecycle)
  phase_1   Happy path (Issue #1: Idle → Completed)
  phase_2   PR close recovery (Issue #2: ReviewWaiting → re-run → Completed)
  phase_3   Cancel + reopen (Issue #3: cancel → Cancelled → reopen → Completed)
  phase_4   Retry + cleanup (Issue #4: fake-claude → Cancelled → cupola cleanup)
  phase_5   Orphan recovery (Issue #5: kill -9 → restart → orphan detection)
  phase_6   Compress (summarize completed specs)
  phase_7   Teardown (cupola stop)

Examples:
  run.sh                              # Full run: phase_0 through phase_7
  run.sh --only phase_0               # Run only phase_0 (pre-flight)
  run.sh --from phase_3               # Resume from phase_3
  run.sh --keep-repo                  # Preserve repo on success
  run.sh --reuse-repo owner/cupola-e2e-20260408-153012-abc --only phase_5
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
FROM_PHASE=""
ONLY_PHASE=""
TO_PHASE=""

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
    --from)
      FROM_PHASE="$2"; shift 2 ;;
    --only)
      ONLY_PHASE="$2"; shift 2 ;;
    --to)
      TO_PHASE="$2"; shift 2 ;;
    --reuse-repo)
      REUSE_REPO="$2"; shift 2 ;;
    --delete-repo)
      DELETE_REPO="$2"; shift 2 ;;
    --owner)
      OWNER="$2"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    -*)
      printf "Unknown option: %s\n" "$1" >&2
      usage
      exit 1
      ;;
    *)
      printf "Unknown argument: %s\n" "$1" >&2
      usage
      exit 1
      ;;
  esac
done

# Validate --from / --only values
_validate_phase_name() {
  local name="$1"
  case "$name" in
    phase_0|phase_1|phase_2|phase_3|phase_4|phase_5|phase_6|phase_7)
      return 0 ;;
    *)
      printf "Invalid phase name: %s\nValid phases: phase_0 phase_1 phase_2 phase_3 phase_4 phase_5 phase_6 phase_7\n" "$name" >&2
      exit 1 ;;
  esac
}

if [ -n "$FROM_PHASE" ]; then
  _validate_phase_name "$FROM_PHASE"
fi
if [ -n "$ONLY_PHASE" ]; then
  _validate_phase_name "$ONLY_PHASE"
fi
if [ -n "$TO_PHASE" ]; then
  _validate_phase_name "$TO_PHASE"
fi

export KEEP_REPO NO_KEEP_DIR FAIL_FAST REUSE_REPO OWNER FROM_PHASE ONLY_PHASE TO_PHASE

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
# Set up run directory
# ---------------------------------------------------------------------------
RUN_BASE="${HOME}/work/cupola-e2e-run"
mkdir -p "$RUN_BASE"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RUN_SUFFIX=$(random3)
RUN_DIR="${RUN_BASE}/${TIMESTAMP}-${RUN_SUFFIX}"
mkdir -p "$RUN_DIR"
export RUN_DIR SCRIPT_DIR

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
# Source narrative scenario file
# ---------------------------------------------------------------------------
# shellcheck source=scenarios/narrative.sh
. "$SCRIPT_DIR/scenarios/narrative.sh"

# ---------------------------------------------------------------------------
# Build phase list and apply --from / --only filtering
# ---------------------------------------------------------------------------
PHASES=(
  phase_0_preflight
  phase_1_happy_path
  phase_2_pr_close_recovery
  phase_3_cancel_reopen
  phase_4_retry_and_cleanup
  phase_5_orphan_recovery
  phase_6_compress
  phase_7_teardown
)

# Map short phase name (phase_N) to full function name
_phase_fn() {
  local short="$1"
  local fn
  for fn in "${PHASES[@]}"; do
    if printf '%s' "$fn" | grep -q "^${short}_"; then
      printf '%s\n' "$fn"
      return 0
    fi
    # Also allow exact match (e.g. phase_0_preflight)
    if [ "$fn" = "$short" ]; then
      printf '%s\n' "$fn"
      return 0
    fi
  done
  return 1
}

_phase_index() {
  local target="$1"
  local i=0
  local fn
  for fn in "${PHASES[@]}"; do
    if [ "$fn" = "$target" ] || printf '%s' "$fn" | grep -q "^${target}_"; then
      printf '%s\n' "$i"
      return 0
    fi
    i=$((i + 1))
  done
  return 1
}

NUM_PHASES=${#PHASES[@]}
start_index=0
end_index=$((NUM_PHASES - 1))

if [ -n "${ONLY_PHASE:-}" ]; then
  idx=$(_phase_index "$ONLY_PHASE") || {
    log_error "Cannot find phase: $ONLY_PHASE"
    exit 1
  }
  start_index=$idx
  end_index=$idx
else
  if [ -n "${FROM_PHASE:-}" ]; then
    idx=$(_phase_index "$FROM_PHASE") || {
      log_error "Cannot find phase: $FROM_PHASE"
      exit 1
    }
    start_index=$idx
  fi
  if [ -n "${TO_PHASE:-}" ]; then
    idx=$(_phase_index "$TO_PHASE") || {
      log_error "Cannot find phase: $TO_PHASE"
      exit 1
    }
    end_index=$idx
  fi
fi

# ---------------------------------------------------------------------------
# Initialize result.json
# ---------------------------------------------------------------------------
result_start

# ---------------------------------------------------------------------------
# Run phases
# ---------------------------------------------------------------------------
FAILED=0
i=$start_index
while [ "$i" -le "$end_index" ]; do
  phase="${PHASES[$i]}"
  log_section "Running $phase"

  phase_rc=0
  "$phase" || phase_rc=$?

  if [ "$phase_rc" -eq 0 ]; then
    log_info "Phase $phase: PASSED"
  elif [ "$phase_rc" -eq 2 ]; then
    log_error "Phase $phase: ABORTED (fail-fast)"
    FAILED=1
    EXIT_CODE=1
    break
  else
    log_error "Phase $phase: FAILED (rc=${phase_rc})"
    FAILED=1
    if [ "${FAIL_FAST:-0}" = "1" ]; then
      log_error "--fail-fast: stopping after phase failure."
      EXIT_CODE=1
      break
    fi
  fi

  i=$((i + 1))
done

# ---------------------------------------------------------------------------
# Final exit code
# ---------------------------------------------------------------------------
if [ "$FAILED" -ne 0 ]; then
  EXIT_CODE=1
fi

# Also fail if any CP within result.json failed, even when phase rc==0.
if [ -f "$RUN_DIR/result.json" ]; then
  if command -v jq >/dev/null 2>&1; then
    _fail_cp=$(jq '[.scenarios[] | select(.status == "fail")] | length' "$RUN_DIR/result.json" 2>/dev/null || echo 0)
  else
    _fail_cp=$(grep -o '"status":"fail"' "$RUN_DIR/result.json" | wc -l | tr -d ' ')
  fi
  if [ -n "$_fail_cp" ] && [ "$_fail_cp" -gt 0 ] 2>/dev/null; then
    EXIT_CODE=1
  fi
fi

exit "$EXIT_CODE"
