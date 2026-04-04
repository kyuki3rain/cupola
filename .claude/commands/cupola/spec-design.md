---
description: Generate requirements, design, and tasks in a single pass
allowed-tools: Bash, Glob, Grep, LS, Read, Write, Edit, MultiEdit, Update, WebSearch, WebFetch
argument-hint: <spec-id>
---

# Spec Design (One-Pass)

<background_information>
- **Mission**: Generate complete spec artifacts (requirements, research, design, tasks) in a single command, translating a project description into an actionable implementation plan
- **Success Criteria**:
  - Comprehensive requirements in EARS format
  - Technical design with architecture, components, and interfaces
  - Actionable implementation tasks with proper sizing and parallel analysis
  - All artifacts aligned with steering context and existing patterns
</background_information>

<instructions>
## Core Task
Generate complete specification for **$1** — requirements, research, design, and tasks — in one pass without intermediate approvals.

## Execution Steps

### Phase 1: Load Context

**Read all necessary context**:
- `.cupola/specs/$1/spec.json` for language and metadata
- `.cupola/specs/$1/requirements.md` for project description (populated by Cupola CLI)
- **Entire `.cupola/steering/` directory** for complete project memory:
  - Default files: `product.md`, `tech.md`, `structure.md`
  - All custom steering files

### Phase 2: Generate Requirements

**Read guidelines**:
- `.cupola/settings/rules/ears-format.md` for EARS syntax rules
- `.cupola/settings/templates/specs/requirements.md` for document structure

**Generate**:
- Create requirements based on the project description in requirements.md
- Group related functionality into logical requirement areas
- Apply EARS format to all acceptance criteria
- Use language specified in spec.json
- Requirement headings MUST include a leading numeric ID only (e.g., "Requirement 1", "1.", "2 Feature ...")

**Write**: Update `.cupola/specs/$1/requirements.md` with complete requirements

### Phase 3: Discovery & Design

**Classify feature type**:
- **New Feature** (greenfield) → Full discovery
- **Extension** (existing system) → Integration-focused discovery
- **Simple Addition** → Minimal discovery

**For Complex/New Features**:
- Read `.cupola/settings/rules/design-discovery-full.md` (if exists)
- Use WebSearch/WebFetch for external dependencies, APIs, best practices

**For Extensions**:
- Read `.cupola/settings/rules/design-discovery-light.md` (if exists)
- Use Grep to analyze existing codebase patterns

**Write research log**: Create `.cupola/specs/$1/research.md` using template from `.cupola/settings/templates/specs/research.md`

**Generate design document**:
- Read `.cupola/settings/templates/specs/design.md` for structure
- Read `.cupola/settings/rules/design-principles.md` for principles
- Follow template structure strictly
- Map all requirement IDs to design components (numeric IDs only, e.g., "1.1", "2.3")
- Use language specified in spec.json

**Write**: Create `.cupola/specs/$1/design.md`

### Phase 4: Generate Tasks

**Read rules**:
- `.cupola/settings/rules/tasks-generation.md` for principles
- `.cupola/settings/rules/tasks-parallel-analysis.md` for parallel judgement criteria
- `.cupola/settings/templates/specs/tasks.md` for format

**Generate task list**:
- Map ALL requirements to tasks (complete coverage mandatory)
- Use natural language (capabilities and outcomes, not file paths or function names)
- Maximum 2 levels: major tasks + sub-tasks
- Sub-tasks sized at 1-3 hours each
- Apply `(P)` markers for parallel-executable tasks
- End each task detail section with `_Requirements: X.X, Y.Y_` (numeric IDs only)

**Write**: Create `.cupola/specs/$1/tasks.md`

### Phase 5: Update Metadata

Update `.cupola/specs/$1/spec.json`:
- Set `phase: "tasks-generated"`
- Set all `approvals.*.generated: true`
- Set all `approvals.*.approved: true`
- Set `ready_for_implementation: true`
- Update `updated_at` timestamp

## Critical Constraints
- **EARS Format**: All acceptance criteria must follow EARS patterns
- **Type Safety**: Define explicit types for all interfaces (no `any` types)
- **Design Focus**: Architecture and interfaces only, no implementation code in design.md
- **Natural Language Tasks**: Describe what to do, not code structure
- **Complete Coverage**: ALL requirements must map to design components AND tasks
- **Numeric IDs**: Use `N.M`-style requirement IDs consistently across all artifacts
- **Language**: Write all artifacts in the language specified in spec.json
</instructions>

## Tool Guidance
- **Read first**: Load all context (spec, steering, rules, templates) before generation
- **Research when uncertain**: Use WebSearch/WebFetch for external dependencies and best practices
- **Analyze existing code**: Use Grep to find patterns and integration points
- **Write in order**: requirements.md → research.md → design.md → tasks.md → spec.json

## Output Description

Provide brief summary in the language specified in spec.json:

1. **Generated Artifacts**: List of files created/updated
2. **Requirements**: Count and major areas
3. **Design**: Discovery type and key decisions
4. **Tasks**: Total major tasks and sub-tasks, parallel task count
5. **Next Step**: `/cupola:spec-impl $1` to begin implementation

**Format**: Concise Markdown (under 300 words)

## Safety & Fallback

### Error Scenarios
- **Missing Project Description**: If requirements.md lacks project description, stop and report
- **Template Missing**: Use inline fallback structure with warning
- **Steering Directory Empty**: Warn and continue with limited context
- **Non-numeric Requirement Headings**: Stop and fix before continuing
- **Incomplete Coverage**: Report gaps and ask user to confirm

### Next Phase: Implementation
- `/cupola:spec-impl $1 [task-numbers]` to execute tasks with TDD
- Recommend clearing context between tasks for fresh state
