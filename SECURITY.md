# Security Policy

## Prompt Injection Risk

Cupola passes GitHub Issue bodies and PR review comments directly to Claude Code as input.
In public repositories, malicious users could craft Issue or PR content designed to manipulate
the agent's behavior — this is known as a **prompt injection attack**.

### Why This Matters

If an attacker can make the agent execute arbitrary commands or generate harmful code by embedding
instructions in Issue/PR content, the integrity of your repository and CI/CD pipeline could be
compromised.

## `trusted_associations` Feature

Cupola mitigates prompt injection risk by **authenticating the source of input** rather than
sanitizing the content. The `trusted_associations` configuration controls which GitHub users
are allowed to trigger the agent.

### How It Works

1. **`agent:ready` label check**: When an Issue is labeled with `agent:ready`, Cupola checks
   who applied the label using the GitHub Timeline API. It then fetches that user's repository
   permission level via the Collaborators API and maps it to an `author_association` value.

2. **Review comment filtering**: When writing `review_threads.json` for Claude Code, Cupola
   filters out comments from authors whose `author_association` is not in `trusted_associations`.

### Configuration

In your `cupola.toml`:

```toml
# Default (secure): only repository owners, members, and collaborators can trigger the agent
trusted_associations = ["OWNER", "MEMBER", "COLLABORATOR"]

# For private repositories where all users are trusted:
# trusted_associations = ["all"]
```

### Valid Association Values

| Value | Description |
|-------|-------------|
| `OWNER` | Repository owner |
| `MEMBER` | Organization member |
| `COLLABORATOR` | Direct collaborator (write/maintain/admin access) |
| `CONTRIBUTOR` | Has made prior contributions |
| `FIRST_TIMER` | First-time contributor |
| `FIRST_TIME_CONTRIBUTOR` | First-time contributor to this repo |
| `NONE` | No special relationship |

### Recommendations

- **Default setting** (`["OWNER", "MEMBER", "COLLABORATOR"]`) is recommended for public
  repositories. This ensures only trusted maintainers can trigger the agent.

- **`"all"` setting** should only be used for private repositories where all users are
  trusted. Using this setting on a public repository removes the prompt injection protection.

- **Never use `"all"` on a public repository** unless you fully understand and accept the
  prompt injection risk.

### What Happens When a User is Rejected

When Cupola detects that the `agent:ready` label was applied by a user without a trusted
association:

1. The `agent:ready` label is removed from the Issue.
2. A comment is posted to the Issue explaining why the label was removed and what
   association levels are trusted.
3. The Issue is skipped in the current polling cycle.

## Issue Body Approval and Tampering Detection

### The Trust Model

The `agent:ready` label acts as an **approval gate**: the person who applies the label
takes responsibility for approving the Issue body content **at the time of labeling**.

Key points:

- **Issue authors can always edit their own Issue body**, regardless of their
  `author_association`. A user with `NONE` association who creates an Issue can modify
  its content even after a trusted collaborator applies `agent:ready`.

- **Collaborators and higher also have edit permissions** on Issue bodies. Granting
  collaborator status to a user effectively extends the trust boundary to them.

### Hash-Based Tampering Detection

To prevent post-approval body modifications from reaching Claude Code, Cupola implements
SHA-256 hash-based body tampering detection:

1. **Snapshot on approval**: When `SpawnInit` runs (triggered by `agent:ready`), Cupola
   fetches the current Issue body and computes its SHA-256 hex digest. This hash is stored
   as the **approval snapshot**.

2. **Verification before each spawn**: Before each subsequent Claude Code spawn
   (`SpawnProcess`), Cupola re-fetches the Issue body and recomputes the hash. If the hash
   differs from the stored snapshot, the body has been modified.

3. **Automatic cancellation on detection**: If tampering is detected, Cupola:
   - Transitions the Issue state to `Cancelled` in the database
   - Removes the `agent:ready` label (best-effort; failure is logged as a warning)
   - Posts a notification comment explaining the cancellation and how to resume (best-effort)
   - Emits a `warn!` trace event with the Issue number

### Re-approval Flow

If an Issue body change is intentional and acceptable, a trusted user can resume processing:

1. Review the **current** Issue body content.
2. If the content is acceptable, re-apply the `agent:ready` label.
3. Cupola will run `SpawnInit` again, fetching the current body and saving a new approval
   snapshot. Subsequent spawns will be validated against this new snapshot.

### Important Notes

- **`body_hash = NULL` issues are backward-compatible**: Issues initialized before this
  feature was introduced will have no stored hash. For these issues, hash comparison is
  skipped and processing continues normally. The hash will be set on the next `SpawnInit`
  (i.e., when `agent:ready` is re-applied).

- **In-flight changes are not covered**: Only changes made *between* `SpawnInit` and a
  subsequent `SpawnProcess` are detected. Changes made during an active Claude Code session
  are not intercepted (the session uses already-written input files).

- **PR review comments are handled separately** via the per-author trust check
  (`trusted_associations`). Do not use Issue body edits to send change requests to the
  agent; use PR review comments instead.

## Reporting Security Vulnerabilities

Please report security vulnerabilities by opening a GitHub Issue with the `security` label,
or by contacting the maintainers directly.
