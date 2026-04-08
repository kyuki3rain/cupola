#!/usr/bin/env bash
# lib/repo.sh — create / teardown ephemeral GitHub repositories
set -euo pipefail
IFS=$'\n\t'

create_ephemeral_repo() {
  log_section "Creating ephemeral repository"

  # Resolve owner
  if [ -z "${OWNER:-}" ]; then
    OWNER=$(gh api user --jq .login)
  fi
  export OWNER
  log_info "Owner: $OWNER"

  # Generate unique name with collision check (up to 3 retries)
  local ts suffix name
  ts=$(date +%Y%m%d-%H%M%S)
  local attempt=0
  while true; do
    suffix=$(random3)
    name="cupola-e2e-${ts}-${suffix}"
    if ! gh repo view "$OWNER/$name" >/dev/null 2>&1; then
      break
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 3 ]; then
      log_error "Could not generate a unique repo name after 3 attempts."
      exit 1
    fi
    log_warn "Name collision: $OWNER/$name. Retrying..."
  done

  export REPO_NAME="$name"
  export REPO_OWNER="$OWNER"
  export REPO_FULL="$OWNER/$name"
  log_info "Repo name: $REPO_FULL"

  # Create repo
  gh repo create "$REPO_FULL" \
    --private \
    --add-readme \
    --description "cupola E2E ephemeral sandbox"
  log_info "Repository created: $REPO_FULL"

  # Clone into RUN_DIR/target
  export TARGET_DIR="$RUN_DIR/target"
  gh repo clone "$REPO_FULL" "$TARGET_DIR"
  log_info "Cloned to: $TARGET_DIR"

  # Copy seed files (including .github)
  local seed_dir
  seed_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/seed"
  if [ -d "$seed_dir" ]; then
    cp -R "$seed_dir/." "$TARGET_DIR/"
    log_info "Seed files copied."
  else
    log_warn "Seed directory not found: $seed_dir"
  fi

  # Initial commit and push
  (
    cd "$TARGET_DIR"
    git add -A
    git -c user.email=e2e@cupola.local -c user.name="cupola-e2e" \
      commit -m "init: seed" --allow-empty
    git push
  )
  log_info "Initial commit pushed."

  # Create labels
  gh label create "agent:ready"  --repo "$REPO_FULL" --color "0075ca" --force || true
  gh label create "weight:light" --repo "$REPO_FULL" --color "e4e669" --force || true
  gh label create "weight:heavy" --repo "$REPO_FULL" --color "d93f0b" --force || true
  log_info "Labels created."

  # Write repo name file
  printf '%s\n' "$REPO_FULL" > "$RUN_DIR/repo-name.txt"
  log_info "Repo name written to $RUN_DIR/repo-name.txt"
}

teardown_repo() {
  local exit_code="${1:-1}"

  log_section "Teardown"

  # Stop cupola if running
  if [ -n "${TARGET_DIR:-}" ] && [ -d "${TARGET_DIR:-}" ]; then
    (cd "$TARGET_DIR" && "$CUPOLA_BIN" stop 2>/dev/null || true)
    log_info "cupola stopped (or was not running)."

    # Preserve logs and DB
    local preserved_dir="$RUN_DIR/preserved"
    mkdir -p "$preserved_dir"
    if [ -d "$TARGET_DIR/.cupola/logs" ]; then
      cp -R "$TARGET_DIR/.cupola/logs" "$preserved_dir/" 2>/dev/null || true
    fi
    if [ -f "$TARGET_DIR/.cupola/cupola.db" ]; then
      cp "$TARGET_DIR/.cupola/cupola.db" "$preserved_dir/" 2>/dev/null || true
    fi
    log_info "Artifacts preserved to $preserved_dir"
  fi

  # Decide whether to delete repo
  if [ "$exit_code" -eq 0 ] && [ "${KEEP_REPO:-0}" != "1" ] && [ -z "${REUSE_REPO:-}" ]; then
    if [ -n "${REPO_FULL:-}" ]; then
      log_info "Deleting ephemeral repo: $REPO_FULL"
      gh repo delete "$REPO_FULL" --yes || log_warn "Failed to delete repo $REPO_FULL"
    fi
    if [ "${NO_KEEP_DIR:-0}" = "1" ] && [ -n "${RUN_DIR:-}" ] && [ -d "${RUN_DIR:-}" ]; then
      rm -rf "$RUN_DIR"
      log_info "Run directory removed."
    fi
  else
    # Print preservation notice
    printf "\n" >&2
    log_warn "Repo is preserved for inspection:"
    if [ -n "${REPO_FULL:-}" ]; then
      printf "  https://github.com/%s\n" "$REPO_FULL" >&2
    fi
    log_warn "Run directory:"
    printf "  %s\n" "${RUN_DIR:-<unknown>}" >&2
    log_warn "To clean up later:"
    if [ -n "${REPO_FULL:-}" ]; then
      printf "  scripts/e2e/run.sh --delete-repo %s\n" "$REPO_FULL" >&2
    fi
  fi
}
