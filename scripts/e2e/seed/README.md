# cupola-e2e-todo

A minimal TypeScript TODO CLI used as the seed project for Cupola E2E tests.

## What this is

This repository is created fresh for each Cupola E2E test run as an ephemeral GitHub sandbox. Cupola (the AI-driven spec and implementation agent) runs against this project to validate its full workflow.

## Running tests

```bash
npm install
npm test
```

## Building

```bash
npm run build
# Then: node dist/index.js <command>
```

## Commands

```bash
# Add a task
npx tsx src/index.ts add "buy milk"

# List tasks
npx tsx src/index.ts list

# Mark done
npx tsx src/index.ts done 1
```

## Cupola docs

See the [Cupola project](https://github.com/kyuki3rain/cupola) for details on the E2E test infrastructure.
