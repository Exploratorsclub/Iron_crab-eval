# AGENTS.md

## Cursor Cloud specific instructions

This is a pure Rust test suite (blackbox + invariant tests) for the IronCrab Solana trading bot. It contains no runnable application — only `cargo test` is the main entry point.

### Build & Test

```bash
cargo fmt -p ironcrab-eval -- --check
cargo clippy -p ironcrab-eval --all-targets -- -D warnings
cargo test --verbose
```

### Dependency

The `ironcrab` crate is pulled from GitHub via git dependency. No sibling repo checkout needed — Cargo fetches it automatically.

---

## Mandatory Rules (STOP-CHECK)

**THESE RULES ARE BINDING. THEY ARE NOT SUGGESTIONS.**

Before modifying ANY file, run ALL checks below. If any check fails: **STOP IMMEDIATELY**, do NOT make the change, and report the violation.

### Check 1: Scope — Eval Repo ONLY

Is the file you want to change inside this repo (`Iron_crab-eval/`)?

- **If no**: STOP IMMEDIATELY. You may ONLY change files in this repo. Report: "STOP: File [X] is outside Iron_crab-eval. Change aborted."

### Check 2: No Implementation Code

Are you writing code that changes implementation logic (not testing it)?

- **If yes**: STOP. You are Test Authority, not Impl Agent.

### Check 3: Repo-Isolation (Level-5 Separation)

Do you read or reference files from `Iron_crab/src/` or `Iron_crab/tests/`?

- **If yes**: STOP IMMEDIATELY. You must NOT read implementation source code. Reading impl code violates Level-5 separation and leads to tests tailored to implementation details instead of true blackbox tests.
- **Allowed**: The public API of `ironcrab` (via `use ironcrab::...` in tests) and `Iron_crab/docs/` (INVARIANTS.md, KNOWN_BUG_PATTERNS.md).
- **Forbidden**: `Iron_crab/src/`, `Iron_crab/tests/`

### Check 4: Blackbox Boundary

Does your test enforce implementation details (internal data structures, private methods)?

- **If yes**: STOP. Tests must test at the API boundary (public interface).

### Check 5: Test Assertions Consistency

Do the assertions match the invariant being tested? A test named `no_double_count` that asserts double-counting IS a bug in the test.

- **If contradiction**: STOP and report the contradiction.

**If all 5 checks pass: proceed.**
**After the change: briefly document which checks you performed.**

---

## Context

- This repo contains **Spec** (`docs/spec/`) and **Evaluation Tests** (blackbox + invariants).
- You are the **Test Authority**: you write tests from the spec, no implementation code.

## Allowed

- Read and update spec documents (`docs/spec/`)
- New blackbox scenarios in `tests/`
- New invariant tests in `tests/`
- Reference `ironcrab` API via `use ironcrab::...` (only for testing, not implementing)

## Forbidden

- Changing implementation code in Iron_crab
- Writing tests that enforce implementation details (blackbox = API boundary)
