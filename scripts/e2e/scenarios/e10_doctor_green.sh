#!/usr/bin/env bash
# scenarios/e10_doctor_green.sh — E-10: doctor green
set -euo pipefail
IFS=$'\n\t'

scenario_e10() {
  log_section "E-10: Doctor Green"
  cd "$TARGET_DIR"

  # Run doctor
  local doctor_out doctor_rc
  doctor_rc=0
  doctor_out=$("$CUPOLA_BIN" doctor 2>&1) || doctor_rc=$?

  log_info "doctor exit code: $doctor_rc"
  printf '%s\n' "$doctor_out" >&2

  # 1. Exit code must be 0
  if [ "$doctor_rc" -ne 0 ]; then
    log_error "Expected exit 0 from doctor. Got: $doctor_rc"
    return 1
  fi
  log_info "Step 1 PASSED: exit code 0"

  # 2. stdout contains "Start Readiness" and "Operational Readiness"
  if ! printf '%s' "$doctor_out" | grep -q "Start Readiness"; then
    log_error "Expected 'Start Readiness' in doctor output."
    return 1
  fi
  if ! printf '%s' "$doctor_out" | grep -q "Operational Readiness"; then
    log_error "Expected 'Operational Readiness' in doctor output."
    return 1
  fi
  log_info "Step 2 PASSED: both sections present"

  # 3. No ❌ in stdout
  if printf '%s' "$doctor_out" | grep -q "❌"; then
    log_error "Found ❌ in doctor output (one or more checks failed):"
    printf '%s' "$doctor_out" | grep "❌" >&2
    return 1
  fi
  log_info "Step 3 PASSED: no ❌ found"

  # 4. At least 5 ✅ in stdout
  local check_count
  check_count=$(printf '%s' "$doctor_out" | grep -c "✅" || true)
  log_info "✅ count: $check_count"
  if [ "$check_count" -lt 5 ]; then
    log_error "Expected at least 5 ✅ in doctor output. Got: $check_count"
    return 1
  fi
  log_info "Step 4 PASSED: at least 5 ✅ found"

  log_info "E-10: PASSED"
  return 0
}
