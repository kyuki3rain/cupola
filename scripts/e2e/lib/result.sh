#!/usr/bin/env bash
# lib/result.sh — result.json writer
set -euo pipefail
IFS=$'\n\t'

# ---------------------------------------------------------------------------
# result_start: initialize result.json
# ---------------------------------------------------------------------------
result_start() {
  printf '{"scenarios": []}\n' > "$RUN_DIR/result.json"
}

# ---------------------------------------------------------------------------
# result_append <id> <status:pass|fail> <seconds> <message>
# ---------------------------------------------------------------------------
result_append() {
  local id="$1"
  local status="$2"
  local seconds="$3"
  local message="$4"

  local current_json
  current_json=$(cat "$RUN_DIR/result.json")

  if command -v jq >/dev/null 2>&1; then
    local entry
    entry=$(jq -n \
      --arg id "$id" \
      --arg status "$status" \
      --argjson seconds "$seconds" \
      --arg message "$message" \
      '{id: $id, status: $status, seconds: $seconds, message: $message}')
    printf '%s' "$current_json" | jq \
      --argjson entry "$entry" \
      '.scenarios += [$entry]' \
      > "$RUN_DIR/result.json"
  else
    # Fallback: naive append without jq
    # Remove trailing `]}` and append new entry
    local base
    base=$(printf '%s' "$current_json" | sed 's/\]\}$//')
    # Check if scenarios array is empty
    if printf '%s' "$base" | grep -q '"scenarios": \[\]'; then
      base=$(printf '%s' "$base" | sed 's/"scenarios": \[\]/"scenarios": [/')
      printf '%s{"id":"%s","status":"%s","seconds":%s,"message":"%s"}]}\n' \
        "$base" "$id" "$status" "$seconds" "$message" \
        > "$RUN_DIR/result.json"
    else
      printf '%s,{"id":"%s","status":"%s","seconds":%s,"message":"%s"}]}\n' \
        "$base" "$id" "$status" "$seconds" "$message" \
        > "$RUN_DIR/result.json"
    fi
  fi
}

# ---------------------------------------------------------------------------
# result_summary: print pass/fail counts
# ---------------------------------------------------------------------------
result_summary() {
  local result_file="$RUN_DIR/result.json"
  if [ ! -f "$result_file" ]; then
    log_warn "No result.json found."
    return 0
  fi

  local pass_count=0 fail_count=0 skip_count=0
  if command -v jq >/dev/null 2>&1; then
    pass_count=$(jq '[.scenarios[] | select(.status == "pass")] | length' "$result_file")
    fail_count=$(jq '[.scenarios[] | select(.status == "fail")] | length' "$result_file")
    skip_count=$(jq '[.scenarios[] | select(.status == "skipped")] | length' "$result_file")
  else
    pass_count=$(grep -o '"status":"pass"' "$result_file" | wc -l | tr -d ' ')
    fail_count=$(grep -o '"status":"fail"' "$result_file" | wc -l | tr -d ' ')
    skip_count=$(grep -o '"status":"skipped"' "$result_file" | wc -l | tr -d ' ')
  fi

  local total=$((pass_count + fail_count + skip_count))
  log_section "Results"
  printf "  Total:   %d\n" "$total" >&2
  printf "  Passed:  %d\n" "$pass_count" >&2
  printf "  Failed:  %d\n" "$fail_count" >&2
  printf "  Skipped: %d\n" "$skip_count" >&2
  printf "  Result: %s\n" "$RUN_DIR/result.json" >&2
}
