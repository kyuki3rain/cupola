---
description: Summarize and archive completed specifications
allowed-tools: Bash, Read, Write, Edit, Glob, Grep
---

# Spec Compress

<background_information>
- **Mission**: Summarize completed specifications into compact archives, reducing context window usage while preserving essential design knowledge
- **Success Criteria**:
  - Completed specs identified and summarized
  - Essential design decisions and architecture preserved in summary
  - Original verbose artifacts removed
  - spec.json updated to archived phase
</background_information>

<instructions>
## Core Task
Scan `.cupola/specs/` for completed specifications, summarize them into `summary.md`, and remove the original verbose artifacts.

## Execution Steps

### Step 1: Identify Completed Specs

1. Scan all directories under `.cupola/specs/`
2. Read each `spec.json` and identify specs where:
   - `phase` is `"implementation-complete"` or similar terminal phase
   - The spec is NOT already `"archived"`
3. If no completed specs found, report and exit

### Step 2: Generate Summary for Each Spec

For each completed spec, read all artifacts and generate a concise `summary.md`:

**Read**:
- `spec.json` (metadata)
- `requirements.md` (what was built)
- `design.md` (how it was designed)
- `tasks.md` (implementation structure)
- `research.md` (design decisions, if exists)

**Generate `summary.md`** with:
1. **Feature**: Name and one-line description
2. **Requirements Summary**: Numbered list of requirement areas (1-2 sentences each)
3. **Architecture Decisions**: Key design choices and rationale (from research.md and design.md)
4. **Components**: List of components introduced or modified
5. **Key Interfaces**: Critical contracts or API changes
6. **Lessons Learned**: Any notable trade-offs, risks encountered, or patterns established

Target length: 100-200 lines (concise but complete)

### Step 3: Archive

For each summarized spec:
1. Write `summary.md` to the spec directory
2. Delete verbose artifacts: `requirements.md`, `design.md`, `tasks.md`, `research.md`
3. Update `spec.json`:
   - Set `phase: "archived"`
   - Update `updated_at` timestamp
4. Keep `spec.json` and `summary.md` only

### Step 4: Report

Output a summary of actions taken.

## Critical Constraints
- **Never archive active specs**: Only process specs with completed implementation
- **Preserve design decisions**: The summary must capture WHY choices were made, not just WHAT was built
- **Idempotent**: Running compress on already-archived specs has no effect
- **Language**: Write summary in the language specified in spec.json
</instructions>

## Tool Guidance
- Use **Glob** to find spec directories
- Use **Read** to load all spec artifacts
- Use **Write** to create summary.md
- Use **Bash** to remove archived files (rm)
- Use **Edit** to update spec.json

## Output Description

Provide brief summary:

1. **Archived**: Count of specs archived
2. **Skipped**: Count of specs already archived or still active
3. **Space Saved**: Approximate line count reduction

**Format**: Concise (under 100 words)

## Safety & Fallback

### Error Scenarios
- **No completed specs**: Report "No completed specs found to archive" and exit normally
- **Malformed spec.json**: Skip the spec with a warning
- **Permission error**: Report and skip the affected spec
