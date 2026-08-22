# Definition of Done (aktuell)

**Stand:** 2026-08-22

Die alte 700-Zeilen-Umbau-Checkliste (Sniper, `KEYPAIR_PATH`, fehlendes `arb-strategy` / `position-manager`) ist **nicht** mehr der Arbeitsstand. Sie liegt in der Git-History vor diesem Commit.

Aktuelle Abnahme:

1. **Invarianten:** `docs/spec/INVARIANTS.md` — Verstoß = nicht done.
2. **Rollen:** nur `execution-engine` hat Keys. `position-manager` schreibt allein KV `POSITION_AUTHORITY`.
3. **CI Impl (`architecture-rebuild` / PR):** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, plus Workflow **Eval (Level 5)** gegen aktuelles `ironcrab-eval`.
4. **CI Eval-PR (`main`):** `cargo fmt -p ironcrab-eval -- --check`, `cargo check`/`cargo build`, `cargo clippy -p ironcrab-eval` **ohne** `--all-targets`. Kein `cargo test` in diesem Gate.
5. **Fachliche Eval-Suite:** Impl Eval Level 5 oder Workflow „Eval invariant tests (manual)“.
6. **Merge:** CI grün, Supervisor-Review, Bugbot ohne offene Issues (siehe Workspace-Supervisor-Regel).
7. **Deploy:** nur mit ausdrücklicher Maintainer-Freigabe.

Onboarding: `CONTRIBUTING.md`.
