# Handoff: INVARIANTS D.2 – Execution Finality Consistency

**Erstellt:** 2026-03-04 | **Quelle:** INVARIANTS.md D.2, User-Entscheidung

---

## 1. Aufgabe

**Invariante D.2:** Position darf nur aus **FINALIZED** executions entstehen (nicht confirmed).

Aktuell wird auf `confirmed` gewartet. Bei Reorg/Fork auf Solana kann eine confirmed TX noch zurückgenommen werden. `finalized` ist sicher.

---

## 2. Spec-Referenz

- `Iron_crab-eval/docs/spec/INVARIANTS.md` §D.2
- `Iron_crab-eval/docs/spec/DEFINITION_OF_DONE.md` (Decision Records, Sim-Gate)

---

## 3. Erlaubte Dateien (Impl Agent)

- `Iron_crab/src/bin/execution_engine.rs`
- `Iron_crab/src/solana/geyser_tx_confirm.rs`
- `Iron_crab/src/config.rs` (ExecutionEngineCfg)
- `Iron_crab/docs/CONFIG_SCHEMA.md` (Dokumentation)

---

## 4. Implementierungsvorgaben

### 4.1 Config

- Neues Feld `confirm_commitment: String` in `ExecutionConfig` (execution_engine.rs)
- Default: `"finalized"`
- TOML: `[execution_engine] confirm_commitment = "finalized"` (optional, in config.rs ExecutionEngineCfg)
- Hot-reload: `confirm_commitment` über Config-Update (RPC-Pfad nutzt neuen Wert; Geyser nutzt Startup-Wert)

### 4.2 Geyser TX Confirmation

- `geyser_tx_confirm.rs`: `with_geyser(..., confirm_commitment: &str)` erweitern
- `build_tx_status_subscribe_request`: `commitment` Parameter nutzen statt hardcoded `Confirmed`
- `CommitmentLevel::Finalized` wenn `confirm_commitment` "finalized" (oder default)

### 4.3 RPC Polling Fallback

- `confirm_via_rpc_polling`: Wenn `config.confirm_commitment == "finalized"` → nur `TransactionConfirmationStatus::Finalized` akzeptieren
- Bei `"confirmed"`: weiterhin Confirmed und Finalized akzeptieren (Rückwärtskompatibilität)

### 4.4 Hinweise

- INVARIANTS.md, KNOWN_BUG_PATTERNS.md prüfen
- Kein RPC im Hot Path (I-4, I-7) – Bestätigung ist nach Send, nicht im Hot Path

---

## 5. Delegation

```
cd Iron_crab && agent -p "Implementiere INVARIANTS D.2 Execution Finality Consistency. Siehe Iron_crab-eval/docs/supervisor/context_d2_execution_finality.md. Position nur aus finalized executions. Config confirm_commitment (default finalized). Geyser + RPC-Polling anpassen."
```
