#!/usr/bin/env bash
# lib/cupola.sh — cupola init, state waiter, PR waiter, sqlite helper
set -euo pipefail
IFS=$'\n\t'

# ---------------------------------------------------------------------------
# init_cupola: run `cupola init`, render toml template, verify `doctor`
# ---------------------------------------------------------------------------
init_cupola() {
  log_section "Initializing cupola"
  cd "$TARGET_DIR"

  # Run cupola init
  local init_out
  init_out=$("$CUPOLA_BIN" init 2>&1) || {
    log_error "cupola init failed:"
    printf '%s\n' "$init_out" >&2
    return 1
  }
  log_info "cupola init: OK"

  # Render cupola.toml from template
  local tmpl_path
  tmpl_path="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/fixtures/cupola.toml.tmpl"
  local toml_dest="$TARGET_DIR/.cupola/cupola.toml"

  if [ ! -f "$tmpl_path" ]; then
    log_error "Template not found: $tmpl_path"
    return 1
  fi

  local repo_only="${REPO_NAME:-${REPO_FULL##*/}}"
  sed \
    -e "s|__OWNER__|${OWNER:-}|g" \
    -e "s|__REPO__|${repo_only}|g" \
    "$tmpl_path" > "$toml_dest"
  log_info "cupola.toml rendered: $toml_dest"

  # Seed a minimal steering file so `doctor` passes (init creates the dir empty).
  local steering_dir="$TARGET_DIR/.cupola/steering"
  mkdir -p "$steering_dir"
  if [ -z "$(ls -A "$steering_dir" 2>/dev/null)" ]; then
    cat > "$steering_dir/product.md" <<'EOF'
# Product (E2E placeholder)

Minimal steering file seeded by the E2E runner so `cupola doctor` passes.
Target system: TypeScript TODO CLI.
EOF
    log_info "Seeded placeholder steering/product.md"
  fi

  # Verify doctor
  local doctor_out
  if doctor_out=$("$CUPOLA_BIN" doctor 2>&1); then
    log_info "cupola doctor: OK"
  else
    log_error "cupola doctor failed:"
    printf '%s\n' "$doctor_out" >&2
    return 1
  fi
}

# ---------------------------------------------------------------------------
# wait_for_state <issue_number> <expected_state> <timeout_secs>
# Returns 0 if the state is matched within timeout, 1 otherwise.
# ---------------------------------------------------------------------------
wait_for_state() {
  local issue_number="$1"
  local expected_state="$2"
  local timeout_secs="${3:-120}"

  log_info "Waiting for issue #${issue_number} to reach state: ${expected_state} (timeout: ${timeout_secs}s)"

  local elapsed=0
  local interval=5
  local last_state=""

  # Query DB directly — `status` only lists active (non-terminal) issues, so it
  # cannot observe `completed` / `cancelled`. DB is the single source of truth.
  while [ "$elapsed" -lt "$timeout_secs" ]; do
    last_state=$(sqlite_query "SELECT state FROM issues WHERE github_issue_number=${issue_number};" 2>/dev/null || true)
    if [ "$last_state" = "$expected_state" ]; then
      log_info "State reached: #${issue_number} -> ${expected_state} (after ${elapsed}s)"
      return 0
    fi
    sleep "$interval"
    elapsed=$((elapsed + interval))
  done

  log_error "Timeout waiting for #${issue_number} to reach ${expected_state} (${timeout_secs}s elapsed)"
  log_error "Last observed DB state: '${last_state}'"
  return 1
}

# ---------------------------------------------------------------------------
# wait_for_pr <issue_number> <type:design|impl> <timeout_secs>
# Echoes the PR number on success.
# ---------------------------------------------------------------------------
wait_for_pr() {
  local issue_number="$1"
  local pr_type="$2"   # design | impl
  local timeout_secs="${3:-900}"

  local branch_suffix
  case "$pr_type" in
    design) branch_suffix="design" ;;
    impl)   branch_suffix="main"   ;;
    *)
      log_error "wait_for_pr: unknown type '$pr_type'. Use 'design' or 'impl'."
      return 1
      ;;
  esac

  local head_branch="cupola/issue-${issue_number}/${branch_suffix}"
  log_info "Waiting for PR from branch: ${head_branch} (timeout: ${timeout_secs}s)"

  local elapsed=0
  local interval=10

  while [ "$elapsed" -lt "$timeout_secs" ]; do
    local pr_json
    pr_json=$(gh pr list \
      --repo "$REPO_FULL" \
      --head "$head_branch" \
      --json number,state \
      --limit 1 2>/dev/null || true)

    local pr_number pr_state
    pr_number=$(printf '%s' "$pr_json" | command -p python3 -c "import sys,json; data=json.load(sys.stdin); print(data[0]['number'] if data else '')" 2>/dev/null || \
                printf '%s' "$pr_json" | sed -n 's/.*"number":[[:space:]]*\([0-9]*\).*/\1/p' | head -1)
    pr_state=$(printf '%s' "$pr_json" | sed -n 's/.*"state":[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)

    if [ -n "$pr_number" ]; then
      case "$pr_state" in
        OPEN|open|MERGED|merged)
          log_info "PR found: #${pr_number} (state=${pr_state}, elapsed=${elapsed}s)"
          printf '%s\n' "$pr_number"
          return 0
          ;;
      esac
    fi

    sleep "$interval"
    elapsed=$((elapsed + interval))
  done

  log_error "Timeout waiting for PR (type=${pr_type}, issue=#${issue_number}, ${timeout_secs}s elapsed)"
  return 1
}

# ---------------------------------------------------------------------------
# sqlite_query <sql>
# ---------------------------------------------------------------------------
sqlite_query() {
  local sql="$1"
  if ! command -v sqlite3 >/dev/null 2>&1; then
    log_warn "sqlite3 not available, skipping query: $sql"
    return 0
  fi
  sqlite3 "$TARGET_DIR/.cupola/cupola.db" "$sql"
}

# Export helpers so `bash -c '...'` children in narrative.sh can call them.
export -f wait_for_state wait_for_pr sqlite_query 2>/dev/null || true
