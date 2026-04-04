# AI-DLC and Spec-Driven Development

Cupola Spec-Driven Development on AI-DLC (AI Development Life Cycle)

## Project Context

### Paths
- Steering: `.cupola/steering/`
- Specs: `.cupola/specs/`
- ADR: `docs/adr/` (refer to this when making design decisions)

### Steering vs Specification

**Steering** (`.cupola/steering/`) - Guide AI with project-wide rules and context
**Specs** (`.cupola/specs/`) - Formalize development process for individual features

### Active Specifications
- Check `.cupola/specs/` for active specifications
- Spec id follows `issue-{number}` naming (created by Cupola CLI)

## Development Guidelines
- Think in English, generate responses in Japanese. All Markdown content written to project files (e.g., requirements.md, design.md, tasks.md, research.md, validation reports) MUST be written in the target language configured for this specification (see spec.json.language).

## Minimal Workflow
- Phase 0 (optional): `/cupola:steering`
- Phase 1 (Specification): `/cupola:spec-design {spec-id}` (generates requirements + design + tasks in one pass)
- Phase 2 (Implementation): `/cupola:spec-impl {spec-id} [tasks]`
- Archival: `/cupola:spec-compress` (summarize completed specs)

## Development Rules
- Design artifacts are reviewed via PR (single approval gate)
- Follow the user's instructions precisely, and within that scope act autonomously: gather the necessary context and complete the requested work end-to-end in this run, asking questions only when essential information is missing or the instructions are critically ambiguous.

## Steering Configuration
- Load entire `.cupola/steering/` as project memory
- Default files: `product.md`, `tech.md`, `structure.md`
- Custom files are supported (managed via `/cupola:steering`)
