#!/usr/bin/env bash
# scenarios/e02_daemon_lifecycle.sh — E-02: Daemon lifecycle
set -euo pipefail
IFS=$'\n\t'

scenario_e02() {
  log_section "E-02: Daemon Lifecycle"
  cd "$TARGET_DIR"

  # Ensure cupola is not running before test
  "$CUPOLA_BIN" stop 2>/dev/null || true
  sleep 1

  # 1. Start daemon, verify stdout contains "started cupola (pid="
  local start_out
  start_out=$("$CUPOLA_BIN" start --daemon 2>&1)
  log_info "start --daemon output: $start_out"
  if ! printf '%s' "$start_out" | grep -q "started cupola (pid="; then
    log_error "Expected 'started cupola (pid=...' in output. Got: $start_out"
    return 1
  fi
  log_info "Step 1 PASSED: daemon started"
  sleep 2

  # 2. Start again, expect non-zero exit AND "already running" in stderr/stdout
  local start2_out start2_rc
  start2_rc=0
  start2_out=$("$CUPOLA_BIN" start --daemon 2>&1) || start2_rc=$?
  log_info "start --daemon (2nd) exit=$start2_rc output: $start2_out"
  if [ "$start2_rc" -eq 0 ]; then
    log_error "Expected non-zero exit on second start. Got exit=0."
    "$CUPOLA_BIN" stop || true
    return 1
  fi
  if ! printf '%s' "$start2_out" | grep -qi "already running"; then
    log_error "Expected 'already running' in output. Got: $start2_out"
    "$CUPOLA_BIN" stop || true
    return 1
  fi
  log_info "Step 2 PASSED: double-start rejected"

  # 3. status contains "Process: running (daemon, pid="
  local status_out
  status_out=$("$CUPOLA_BIN" status 2>&1 || true)
  log_info "status output: $status_out"
  if ! printf '%s' "$status_out" | grep -q "Process: running (daemon, pid="; then
    log_error "Expected 'Process: running (daemon, pid=...' in status. Got: $status_out"
    "$CUPOLA_BIN" stop || true
    return 1
  fi
  log_info "Step 3 PASSED: status shows running daemon"

  # 4. stop, verify stdout contains "stopped cupola"
  local stop_out
  stop_out=$("$CUPOLA_BIN" stop 2>&1)
  log_info "stop output: $stop_out"
  if ! printf '%s' "$stop_out" | grep -q "stopped cupola"; then
    log_error "Expected 'stopped cupola' in stop output. Got: $stop_out"
    return 1
  fi
  log_info "Step 4 PASSED: stop succeeded"
  sleep 1

  # 5. status shows "Process: not running"
  local status2_out
  status2_out=$("$CUPOLA_BIN" status 2>&1 || true)
  log_info "status (after stop): $status2_out"
  if ! printf '%s' "$status2_out" | grep -q "Process: not running"; then
    log_error "Expected 'Process: not running' after stop. Got: $status2_out"
    return 1
  fi
  log_info "Step 5 PASSED: status shows not running"

  # 6. PID file should not exist
  local pid_file="$TARGET_DIR/.cupola/cupola.pid"
  if [ -f "$pid_file" ]; then
    log_error "PID file still exists after stop: $pid_file"
    return 1
  fi
  log_info "Step 6 PASSED: PID file removed"

  log_info "E-02: PASSED"
  return 0
}
