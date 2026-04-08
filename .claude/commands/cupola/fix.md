---
description: Address review comments, CI failures, and conflicts for a Cupola-managed PR
allowed-tools: Bash, Glob, Grep, LS, Read, Write, Edit, MultiEdit
argument-hint: <design|impl>
---

# Fix Agent

<background_information>
- **Mission**: Address issues on a Cupola-managed PR — review comments, CI failures, and merge conflicts — and push the fixes
- **Target**: `$1` is either `design` (Design PR) or `impl` (Implementation PR)
- **Success Criteria**:
  - All review threads addressed and replied to (if review_threads.json exists)
  - CI failures resolved (if ci_errors.txt exists)
  - Merge conflicts resolved (if conflict markers exist)
  - Fixes committed and pushed
  - Output schema populated with thread responses
</background_information>

<instructions>
## Core Task

Address outstanding issues on the `$1` PR and push the fixes.

## Step 1: Load Context

Read the following input files to understand what needs fixing:

- `.cupola/inputs/review_threads.json` — review threads to address (may not exist)
- `.cupola/inputs/ci_errors.txt` — CI failure logs (may not exist)
- Check for merge conflict markers (`<<<<<<<`) in working tree files

Also read:
- **Entire `.cupola/steering/` directory** for project context

## Step 2: Resolve Merge Conflicts (if present)

If conflict markers exist in any file:

1. Resolve all conflicts — choose the correct side or merge manually
2. `git add <resolved_files>`
3. `git commit --no-edit`

Resolve conflicts **before** addressing other issues.

## Step 3: Address Review Comments (if review_threads.json exists)

For each thread in `review_threads.json`:

1. Understand the feedback
2. Apply the necessary code changes
3. Record your response and whether the thread should be resolved

## Step 4: Fix CI Failures (if ci_errors.txt exists)

1. Read the CI error log
2. Identify the root cause
3. Apply the fix
4. Verify locally if possible (e.g., `cargo test`, `npm test`)

## Step 5: Quality Check

Run quality checks described in AGENTS.md / CLAUDE.md. Fix any issues before committing.

## Step 6: Commit and Push

```bash
git diff --name-only   # confirm changed files
git add <changed_files>
git commit -m "fix: address requested changes"
git push
```

Stage only relevant files. Do not use `git add -A` or `git add .`.

## Output to output-schema

For each review thread addressed, output:

- `thread_id`: The ID of the thread (use as-is from review_threads.json)
- `response`: Reply content for that thread
- `resolved`: Whether this thread should be resolved (`true` / `false`)

If no review threads exist, return: `{"threads": []}`

Thread replies and resolution will be handled by the system — do not use the GitHub API.

## Constraints

- Do not use the GitHub API (including the `gh` command)
- Do not reply to or resolve threads directly (the system will do it)
- Write all replies in the language specified in `.cupola/specs/*/spec.json` or as instructed
</instructions>

## Tool Guidance

- **Read first**: Load all input files and steering context before making changes
- **Grep**: Use to find conflict markers and locate relevant code sections
- **Test**: Run tests after fixing CI failures to confirm the fix

## Output Description

Provide a brief summary:

1. **Issues addressed**: review threads / CI failures / conflicts handled
2. **Changes made**: files modified
3. **Thread responses**: list of thread IDs and whether resolved
