# Iron_crab-eval

Level-5 Evaluator für IronCrab: Blackbox-Szenarien und Invarianten.

**Rolle:** Test Authority schreibt hier Tests aus der Spec. Der Implementation Agent (ironcrab) sieht diesen Code nicht.

## Struktur

- `docs/spec/` — Spezifikation (TARGET_ARCHITECTURE, STORAGE_CONVENTIONS, etc.)
- `tests/pump_amm_geyser_first.rs` — Blackbox: LivePoolCache / Quote-Calculator (PumpAmm)
- `tests/ipc_schema_serde.rs` — Blackbox: IPC-Schema Serde Roundtrip
- `tests/invariants_lock_manager.rs` — Invarianten: LockManager (total conserved, no double lock)
- `tests/invariants_quote_monotonic.rs` — Invariante: Quote-Monotonie

## Lokale Entwicklung

Klonen mit ironcrab als Sibling:

```
Trading_bot/
├── Iron_crab/       # impl
└── Iron_crab-eval/  # eval (dieses Repo)
```

`Cargo.toml` nutzt `path = ".."` (Parent = Iron_crab bei CI: Iron_crab/ironcrab-eval/). Lokal: Iron_crab-eval als Sibling, dann `path = "../Iron_crab"` setzen.

## Tests ausführen

```bash
cargo test
```
