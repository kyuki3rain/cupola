---
description: Manage .cupola/steering/ as persistent project knowledge
allowed-tools: Bash, Read, Write, Edit, MultiEdit, Glob, Grep, LS
---

# Cupola Steering Management

<background_information>
**Role**: Maintain `.cupola/steering/` as persistent project memory.

**Mission**:
- Bootstrap: Generate core steering from codebase (first-time)
- Sync: Keep steering and codebase aligned (maintenance)
- Preserve: User customizations are sacred, updates are additive

**Success Criteria**:
- Steering captures patterns and principles, not exhaustive lists
- Code drift detected and reported
- All `.cupola/steering/*.md` treated equally (core + custom)
</background_information>

<instructions>
## Scenario Detection

Check `.cupola/steering/` status:

**Bootstrap Mode**: Empty OR missing core files (product.md, tech.md, structure.md)
**Sync Mode**: All core files exist

---

## Bootstrap Flow

1. Load templates from `.cupola/settings/templates/steering/`
2. Analyze codebase (JIT):
   - `glob_file_search` for source files
   - `read_file` for README, package.json, Cargo.toml, etc.
   - `grep` for patterns
3. Extract patterns (not lists):
   - Product: Purpose, value, core capabilities
   - Tech: Frameworks, decisions, conventions
   - Structure: Organization, naming, imports
4. Generate steering files (follow templates)
5. Load principles from `.cupola/settings/rules/steering-principles.md`
6. Present summary for review

**Focus**: Patterns that guide decisions, not catalogs of files/dependencies.

---

## Sync Flow

1. Load all existing steering (`.cupola/steering/*.md`)
2. Analyze codebase for changes (JIT)
3. Detect drift:
   - **Steering → Code**: Missing elements → Warning
   - **Code → Steering**: New patterns → Update candidate
   - **Custom files**: Check relevance
4. Propose updates (additive, preserve user content)
5. Report: Updates, warnings, recommendations

**Update Philosophy**: Add, don't replace. Preserve user sections.

---

## Granularity Principle

From `.cupola/settings/rules/steering-principles.md`:

> "If new code follows existing patterns, steering shouldn't need updating."

Document patterns and principles, not exhaustive lists.

**Bad**: List every file in directory tree
**Good**: Describe organization pattern with examples

</instructions>

## Tool Guidance

- `glob_file_search`: Find source/config files
- `read_file`: Read steering, docs, configs
- `grep`: Search patterns
- `list_dir`: Analyze structure

**JIT Strategy**: Fetch when needed, not upfront.

## Output Description

Chat summary only (files updated directly).

### Bootstrap:
```
Steering Created

## Generated:
- product.md: [Brief description]
- tech.md: [Key stack]
- structure.md: [Organization]

Review and approve as Source of Truth.
```

### Sync:
```
Steering Updated

## Changes:
- tech.md: [What changed]
- structure.md: [What changed]

## Code Drift:
- [Detected issues]

## Recommendations:
- [Suggested improvements]
```

## Safety & Fallback

- **Security**: Never include keys, passwords, secrets
- **Uncertainty**: Report both states, ask user
- **Preservation**: Add rather than replace when in doubt
- Avoid documenting agent-specific tooling directories (e.g., `.cursor/`, `.gemini/`, `.claude/`)
- `.cupola/settings/` content should NOT be documented in steering files (settings are metadata, not project knowledge)
- Light references to `.cupola/specs/` and `.cupola/steering/` are acceptable
