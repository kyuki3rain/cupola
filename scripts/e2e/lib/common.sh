#!/usr/bin/env bash
# lib/common.sh — shared logging helpers, color detection, and guards
set -euo pipefail
IFS=$'\n\t'

# ---------------------------------------------------------------------------
# Color / TTY detection
# ---------------------------------------------------------------------------
_USE_COLOR=0
confirm_tty() {
  if [ -t 2 ]; then
    _USE_COLOR=1
  else
    _USE_COLOR=0
  fi
}
confirm_tty

_clr() {
  local code="$1"; shift
  if [ "$_USE_COLOR" -eq 1 ]; then
    printf "\033[%sm%s\033[0m" "$code" "$*"
  else
    printf "%s" "$*"
  fi
}

# ---------------------------------------------------------------------------
# Logging helpers (all write to stderr)
# ---------------------------------------------------------------------------
log_info() {
  printf "%s [INFO]  %s\n" "$(_clr 36 "$(date '+%H:%M:%S')")" "$*" >&2
}

log_warn() {
  printf "%s [WARN]  %s\n" "$(_clr 33 "$(date '+%H:%M:%S')")" "$(_clr 33 "$*")" >&2
}

log_error() {
  printf "%s [ERROR] %s\n" "$(_clr 31 "$(date '+%H:%M:%S')")" "$(_clr 31 "$*")" >&2
}

log_section() {
  local title="$1"
  printf "\n%s\n%s\n" "$(_clr 35 "=== $title ===")" "" >&2
}

# ---------------------------------------------------------------------------
# random3 — 3-char [a-z0-9]
# ---------------------------------------------------------------------------
random3() {
  # Read a few bytes and filter; avoid `tr | head -c` which trips SIGPIPE under pipefail.
  LC_ALL=C awk 'BEGIN{srand(); c="abcdefghijklmnopqrstuvwxyz0123456789"; for(i=0;i<3;i++) printf "%s", substr(c, int(rand()*36)+1, 1)}'
}

# ---------------------------------------------------------------------------
# Guard: must be run from cupola repo root
# ---------------------------------------------------------------------------
assert_repo_root() {
  if [ ! -f "Cargo.toml" ] || [ ! -f "src/bootstrap/app.rs" ]; then
    log_error "This script must be run from the cupola repository root."
    log_error "Current directory: $PWD"
    log_error "Expected: a directory containing Cargo.toml and src/bootstrap/app.rs"
    exit 1
  fi
}
