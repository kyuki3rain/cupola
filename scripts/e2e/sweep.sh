#!/usr/bin/env bash
# sweep.sh — bulk delete all cupola-e2e-* repos under an owner
set -euo pipefail
IFS=$'\n\t'

OWNER="${1:-$(gh api user --jq .login)}"
printf "Listing cupola-e2e-* repos under %s...\n" "$OWNER"

repos=$(gh repo list "$OWNER" --limit 1000 --json name --jq '.[].name' | grep '^cupola-e2e-' || true)

if [ -z "$repos" ]; then
  printf "No cupola-e2e-* repos found.\n"
  exit 0
fi

printf "Found:\n"
printf '%s\n' "$repos"

# Require interactive confirmation
read -r -p "Delete all? [y/N] " confirm
if [ "$confirm" != "y" ]; then
  printf "Aborted.\n"
  exit 1
fi

while IFS= read -r name; do
  printf "Deleting %s/%s...\n" "$OWNER" "$name"
  gh repo delete "$OWNER/$name" --yes
done <<< "$repos"

printf "Done.\n"
