#!/usr/bin/env bash
set -euo pipefail
# fake-claude-fail.sh — used in Phase 4 to force retry exhaustion
echo "fake-claude: intentional failure for E2E test" >&2
exit 42
