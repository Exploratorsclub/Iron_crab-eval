# Iron_crab-eval

Level-5 Evaluator für IronCrab: Spec, Blackbox-Szenarien und Invarianten.

**Rolle:** Test Authority schreibt hier Tests aus der Spec. Der Implementation Agent (Iron_crab) sieht diesen Test-Code nicht.

**Mitentwickeln:** [CONTRIBUTING.md](CONTRIBUTING.md). Arbeits-Branch ist **`main`**. Die Impl liegt in [Iron_crab](https://github.com/Exploratorsclub/Iron_crab) auf **`architecture-rebuild`**.

## Struktur

- `docs/spec/` — Spezifikation (Einstieg: `INVARIANTS.md`, dann `TARGET_ARCHITECTURE.md`)
- `docs/README.md` — Karte der Docs-Ordner
- `tests/` — Blackbox- und Invarianten-Tests gegen die öffentliche `ironcrab`-API
- `AGENTS.md` — STOP-CHECK für Cloud Agents / Test Authority

## Lokale Entwicklung

Klonen mit Iron_crab als Sibling:

```
Trading_bot/
├── Iron_crab/       # impl, Branch architecture-rebuild
└── Iron_crab-eval/  # eval (dieses Repo)
```

`Cargo.toml` nutzt eine **git-Dependency** auf `ironcrab` (Revision in der Datei). Lokal und in CI überschreibt ein `[patch]` das mit dem Sibling-Pfad — Patch nicht committen. Details: [CONTRIBUTING.md](CONTRIBUTING.md).

## CI

Zwei Gates, nicht verwechseln:

- **Rust** (PR/`main`): `cargo fmt -p ironcrab-eval`, `cargo check`, `cargo build`, `cargo clippy -p ironcrab-eval` **ohne** `--all-targets` und **ohne** `cargo test`. Vermeidet Deadlocks, wenn Eval-Tests eine neuere `ironcrab`-API erwarten als der gerade gepatchte Checkout.
- **Eval invariant tests (manual):** Workflow `eval-invariants-manual.yml` — volle Suite inkl. `clippy --all-targets` und `cargo test`.
- Die **Invarianten gegen den kanonischen Impl-Stand** laufen zusätzlich in **Iron_crab CI** als Job **Eval (Level 5)**.

## Tests ausführen

```bash
cargo test
```

Voraussetzung: passender Iron_crab-Checkout (siehe CONTRIBUTING). Das schlanke PR-Gate ersetzt die volle Suite nicht.
