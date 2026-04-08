#!/usr/bin/env bash
# lib/prereqs.sh — pre-flight environment checks
set -euo pipefail
IFS=$'\n\t'

ensure_prereqs() {
  log_section "Checking prerequisites"

  # 1. gh CLI
  if ! command -v gh >/dev/null 2>&1; then
    log_error "gh CLI not found. Install from https://cli.github.com/"
    exit 1
  fi
  log_info "gh CLI: $(gh --version | head -1)"

  # 2. gh auth status
  if ! gh auth status >/dev/null 2>&1; then
    log_error "gh is not authenticated. Run: gh auth login"
    exit 1
  fi
  log_info "gh auth: OK"

  # 3. delete_repo scope check (skip if --keep-repo)
  if [ "${KEEP_REPO:-0}" != "1" ] && [ -z "${REUSE_REPO:-}" ]; then
    local auth_status
    auth_status=$(gh auth status 2>&1 || true)
    if ! printf '%s' "$auth_status" | grep -q "delete_repo"; then
      log_error "delete_repo scope is required. Run:"
      log_error "  gh auth refresh -h github.com -s delete_repo"
      log_error "Or pass --keep-repo to skip auto-deletion."
      exit 1
    fi
    log_info "delete_repo scope: OK"
  else
    log_info "delete_repo scope: skipped (--keep-repo or --reuse-repo)"
  fi

  # 4. claude CLI
  if ! command -v claude >/dev/null 2>&1; then
    log_warn "claude CLI not found. Some scenarios (E-01) will fail."
    log_warn "Install claude CLI and ensure 'claude --version' works."
  else
    log_info "claude CLI: $(claude --version 2>/dev/null | head -1 || echo 'found')"
  fi

  # 5. git
  if ! command -v git >/dev/null 2>&1; then
    log_error "git not found. Install git 2.40+"
    exit 1
  fi
  log_info "git: $(git --version)"

  # 6. sqlite3
  if ! command -v sqlite3 >/dev/null 2>&1; then
    log_warn "sqlite3 not found. DB verification steps will be skipped."
  else
    log_info "sqlite3: $(sqlite3 --version | head -1)"
  fi

  # 7. cargo / devbox
  local has_cargo=0
  local has_devbox=0
  if command -v cargo >/dev/null 2>&1; then
    has_cargo=1
    log_info "cargo: $(cargo --version)"
  fi
  if command -v devbox >/dev/null 2>&1; then
    has_devbox=1
    log_info "devbox: found"
  fi
  if [ "$has_cargo" -eq 0 ] && [ "$has_devbox" -eq 0 ]; then
    log_error "Neither cargo nor devbox found. Install one to build cupola."
    exit 1
  fi

  # 8. Build cupola binary (cargo no-ops if already up to date)
  log_info "Ensuring cupola release binary is up to date..."
  if [ "$has_devbox" -eq 1 ]; then
    devbox run -- cargo build --release >/dev/null
  else
    cargo build --release >/dev/null
  fi
  export CUPOLA_BIN="$PWD/target/release/cupola"
  log_info "CUPOLA_BIN: $CUPOLA_BIN"

  # 9. Free disk space check (warn if < 2GB; skip on platforms where df behaves differently)
  local free_kb=0
  if df_out=$(df -k . 2>/dev/null); then
    free_kb=$(printf '%s' "$df_out" | awk 'NR==2 {print $4}')
    if [ -n "$free_kb" ] && [ "$free_kb" -lt 2097152 ] 2>/dev/null; then
      log_warn "Low disk space: ${free_kb}KB free. Recommend at least 2GB."
    else
      log_info "Disk space: OK (>= 2GB free)"
    fi
  else
    log_warn "Could not check disk space."
  fi

  log_info "All prerequisite checks passed."
}
