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

## Reporting Security Vulnerabilities

Please report security vulnerabilities by opening a GitHub Issue with the `security` label,
or by contacting the maintainers directly.
