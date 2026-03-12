# AGENTS.md

## Cursor Cloud specific instructions

### Projekt-Übersicht

`ironcrab-eval` ist eine reine Rust-Test-Suite (Blackbox + Invarianten) für das IronCrab Solana Trading-Bot-System. Es enthält keinen ausführbaren Code – nur `cargo test` ist der Haupt-Einstiegspunkt.

### Voraussetzungen

- **Rust stable ≥ 1.85** (wegen `edition2024`-Abhängigkeiten wie `time` crate). VM-Snapshot enthält 1.94.0.
- **`libssl-dev` + `pkg-config`** werden für OpenSSL-sys (transitiv über `solana-sdk`) benötigt. Im Snapshot vorinstalliert.
- **GitHub-Authentifizierung**: `gh auth setup-git` muss gelaufen sein, damit cargo die private Git-Dependency `Exploratorsclub/Iron_crab.git` (Branch `architecture-rebuild`) fetchen kann. Wird im Update-Script ausgeführt.

### Iron_crab Sibling-Verzeichnis

Die Golden-Replay-Tests (`tests/golden_replay_blackbox.rs`) erwarten Iron_crab unter `/Iron_crab` (berechnet als `CARGO_MANIFEST_DIR/../Iron_crab`). Das Update-Script klont dieses Repo automatisch, falls es fehlt.

### CI-Checks (alle 4 müssen bestehen)

Siehe `.github/workflows/rust.yml`. Kurzform:

```bash
cargo fmt -p ironcrab-eval -- --check
cargo check
cargo clippy -p ironcrab-eval --all-targets -- -D warnings
cargo test
```

### Hinweise

- Dies ist ein reines Library-/Test-Crate ohne eigene Binaries. Es gibt keinen `cargo run`-Befehl.
- Die `.cursor/rules/eval-test-authority.mdc` definiert strenge Scope-Regeln: nur Tests schreiben, keinen Impl-Code ändern, kein Lesen von `Iron_crab/src/`.
- Alle 146 Tests sind Blackbox-Tests gegen die öffentliche API von `ironcrab`.
