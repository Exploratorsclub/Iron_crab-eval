# Mitentwickeln an Iron_crab-eval

Stand: 2026-08-22 (GitHub `main` @ `86d67fd`).

Dieses Repo ist **Spec + Test Authority**. Die Implementierung liegt in [Iron_crab](https://github.com/Exploratorsclub/Iron_crab) auf Branch **`architecture-rebuild`** (gemeinsamer Stand). `architecture-rebuild-next` ist nur die aktive Maintainer-Entwicklung. Produktions-Deploy bleibt beim Maintainer.

## Rolle

Du schreibst und pflegst:

- Spezifikation unter `docs/spec/`
- Blackbox- und Invarianten-Tests unter `tests/` gegen die **öffentliche** `ironcrab`-API

Du schreibst **keinen** Implementierungscode und liest **nicht** `Iron_crab/src/` oder `Iron_crab/tests/` (Level-5-Trennung). Erlaubt: öffentliche API (`use ironcrab::...`) und Impl-Docs (`Iron_crab/docs/INVARIANTS.md`, `KNOWN_BUG_PATTERNS.md`).

STOP-CHECK: `AGENTS.md` und `.cursor/rules/eval-test-authority.mdc`.

## Branches

| Repo | Arbeits-Branch |
|------|----------------|
| `Exploratorsclub/Iron_crab-eval` | **`main`** |
| `Exploratorsclub/Iron_crab` | **`architecture-rebuild`** (gemeinsam; `architecture-rebuild-next` = Maintainer) |

PRs in diesem Repo zielen auf `main`.

## Clone (Sibling)

```text
Trading_bot/
├── Iron_crab/
└── Iron_crab-eval/   # dieses Repo
```

`Cargo.toml` zieht `ironcrab` als **git-Dependency** mit gepinnter Revision (aktuell in der Datei nachlesen). Cloud Agents brauchen kein Sibling-Checkout. Lokal und in CI überschreibt ein `[patch]` den Git-Dep mit dem Sibling-Pfad, analog zu `.github/workflows/rust.yml`.

Die Revision in `Cargo.toml` nicht nebenbei anheben. Das ist eine koordinierte Änderung mit einem Impl-Stand auf `architecture-rebuild`.

## Spec-Karte

| Datei | Inhalt |
|--------|--------|
| `docs/spec/INVARIANTS.md` | Lebender Invarianten-Katalog (eval-getestet + Leitlinien). **Zuerst lesen.** |
| `docs/spec/TARGET_ARCHITECTURE.md` | Zielarchitektur; oben steht der aktuelle Betriebsstand. Bei Konflikt gewinnen die Invarianten. |
| `docs/spec/DEFINITION_OF_DONE.md` | Historische Abnahme-Checkliste des Umbaus, kein Tages-Workflow. |
| `docs/spec/ROLE_SEPARATION.md` | Rollen / Keyless |
| `docs/spec/STORAGE_CONVENTIONS.md` | Persistenz / JSONL / Schema |
| `docs/spec/ARB_QUOTE_CONTRACT.md` | Arb-Quote-Vertrag |
| `docs/spec/ARB_TRACK_REQUESTS.md` | Arb Track-Requests |
| `docs/spec/MOMENTUM_ACTIVE_POOLS.md` | Momentum Active Pools |
| `docs/spec/TRAILING_SESSION_HIGH.md` | Trailing Session High |

`docs/plans/` und `docs/supervisor/` sind Arbeitsnotizen des Supervisors, keine Einstiegslektüre.

## Zwei CI-Gates (nicht verwechseln)

**1. Schlankes PR-Gate** — GitHub-Workflow **„Rust“** auf `main`/PRs:

```bash
cargo fmt -p ironcrab-eval -- --check
cargo check
cargo build
cargo clippy -p ironcrab-eval -- -D warnings
```

Absichtlich **ohne** `--all-targets` und **ohne** `cargo test`. Sonst kompilieren Integrationstests gegen die öffentliche Impl-API und blockieren parallele Impl-/Eval-PRs.

**2. Volle Eval-Suite** (fachliche Invarianten):

- Impl-CI-Job **Eval (Level 5)** nach Push/PR auf `architecture-rebuild` (und `architecture-rebuild-next`), oder
- Workflow **„Eval invariant tests (manual)“** (`workflow_dispatch`), oder
- lokal `cargo test` mit passendem Iron_crab-Sibling (idealerweise Tip von `architecture-rebuild`).

Ein Eval-PR gilt nach dem schlanken „Rust“-Workflow als CI-grün. Die fachliche Vollständigkeit hängt an Gate 2.

## Lokale volle Suite

```bash
cd Iron_crab-eval
cat >> Cargo.toml <<'EOF'

[patch."https://github.com/Exploratorsclub/Iron_crab.git"]
ironcrab = { path = "../Iron_crab" }
EOF
cargo test
```

Den Patch nicht committen. `Iron_crab` muss der Stand sein, gegen den die Tests laufen sollen.

## Tests schreiben

- Eine Invariante = klare API-Aussage, keine privaten Felder, keine internen Module.
- Testdatei und Spec-Eintrag in `docs/spec/INVARIANTS.md` zusammen halten.
- Bestehende DEX-Patterns (Raydium / Orca / Meteora / PumpSwap) wiederverwenden, nicht einen Sonderweg erfinden.
- Vor Merge: schlankes Gate lokal fahren. Volle Suite, sobald Impl und Eval zusammenpassen.

## PRs

- Klein, eine Invariante oder ein Spec-Thema pro PR wenn möglich.
- `AGENTS.md` Check 1–5 vor der Änderung durchgehen.

## Was hier nicht dokumentiert ist

Produktions-Deploy, systemd, Live-Keys. Betrieb bleibt beim Maintainer.
