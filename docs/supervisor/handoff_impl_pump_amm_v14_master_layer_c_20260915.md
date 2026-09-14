WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff — Impl: Pump AMM v14 Schicht C bis Pin und ueber JetStream halten

**Repo:** `Exploratorsclub/Iron_crab`  
**Basis:** `architecture-rebuild` @ `4bc28c2` (#445 rustls; Market-Data-Logik = #444 `795fe5b`)  
**Nicht** `architecture-rebuild-next`.  
**Prioritaet:** P0 (Arb: Opportunities ja, Intents nein, weil Pump-Sell ohne DexPoolAccounts)  
**PR-Titel:** `fix(md): keep Pump AMM v14 layer C through pin and JetStream`  
**Plan:** Iron_crab-eval `docs/plans/plan_pump_amm_v14_master_layer_c_20260915.md`  
**Findings:** Iron_crab-eval `docs/supervisor/findings_pump_v14_missing_master_20260915.md`  
**Architektur:** A.52 Addendum Buch vs MASTER; KNOWN_BUG_PATTERNS **#33** und **#28**

**Kein Deploy. Kein Merge auf andere Branches.**

---

## Task-Beschreibung

### Warum C „schon mal da war“ und im MASTER fehlt

Schicht C (`pool_accounts`, Pump: `len>=14` und `accounts[0]==pool`) kommt **nur aus geparsten TX-Instruction-Accounts**. Account-Decode hat die 14 Keys nicht (I-4 / I-12). Quotes brauchen C nicht — deshalb sieht Prod Orca→Pump-Opps mit frischen Vault-Reserves und trotzdem `arb_rejected_missing_accounts`.

Nach A.52 (#441):

- Ungepinnt: TX schreibt C nur ins **Adressbuch** (`apply_tx_pool_accounts_for_hot_pool` → `book.merge`). `md_sidefx_pin_seed_write_pump_layer_c` ist `if !is_hot { return false }`.
- Gepinnt: Fill-Missing MASTER + Pin-Seed-C. Prod seit MD-Restart 22:04: Pump AMM pin-seed written **~15 / 3 h**, `write_miss=0`. Orca-Pin-Seed (~7k) zaehlt Hot-Layout-Apply, nicht dasselbe.
- Buch: `DEFAULT_TTL_MS = 120_000`, `evict_stale` loescht **auch** vollstaendige Pump-v14. Arb pinnt oft spaeter als 120 s nach dem letzten ungepinnten Pump-Trade. Promote kopiert dann Vaults (oder gar nichts) **ohne** C.
- MD-Restart (#444, 22:04): RAM-Buch und RAM-MASTER weg. FIX-33 publiziert C auf JetStream beim Trade; danach ueberschreibt `md_sidefx_build_balance_updated_from_cache` die Last-Message **ohne** `pool_accounts` (Funktion enthaelt 0× diesen Key). Bootstrap hat nichts zum Mergen. #28 Preserve greift nur, wenn schon eine Zeile mit C existiert.

`create_arb_intent` bleibt Geyser-First: beide Beine brauchen DexPoolAccounts; Pump zusaetzlich verifizierte 14. Gate **nicht** lockern.

### Soll (ein PR)

1. **Adressbuch:** `PoolLayoutKeys::PumpAmm` mit `pool_accounts.len() >= 14` nicht per TTL evicten. `DEFAULT_TTL_MS` Zahl unveraendert. Over-Cap: zuerst Eintraege ohne vollstaendige v14. Kein Subscribe, kein MASTER-Upsert ungepinnt.
2. **Promote:** vorhandenes Fill-Missing muss C kopieren (`merge_tx_pool_accounts_into_existing` / `into_layout_only_cached_state` tun das schon, wenn das Buch C noch hat). Unit-Test: Pin nach simulierter >120 s seit v14-Buch-Write → MASTER hat 14 Keys, keine Reserves aus TX.
3. **JetStream:** sobald MASTER C hat, muss **jede** Pump-`BalanceUpdated`/`PoolCacheUpdate` aus dem Cache-Pfad `metadata["pool_accounts"]` im bestehenden comma-joined Format mitfuehren (FIX-26-Pattern aus dem Account-`PoolDiscovered`-Zweig). Trade-FIX-33-Publish behalten. Keine v14 erfinden wenn MASTER `[]`.
4. **Hot-TX:** bestehender Pin-Seed-C-Write bleibt. Nur wenn `MissNoCacheEntry`: Layout-only aus derselben TX, dann C (kein RPC). Non-hot weiter nur Buch.

### Prod-Evidenz (nicht wegargumentieren)

- Zwei-Hop-Opps dominant `orca → pump_amm`, Mint `Ce2gx9KG…pump`, Spread ~930–990 bps.
- `missing_accounts` folgt den Pump-Beinen; Reject ist debug, nicht Journal.
- Completeness Pump: wenige `complete`, viele `missing_vault` — die quotable Pools haben Vaults **ohne** C.
- Einziger Intent der Nacht war orca→DLMM und starb an `post_sim_pnl` — **nicht** dieser PR.

---

## Relevante Invarianten (VOLLTEXT)

### I-4 HOT PATH GEYSER-ONLY
HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. C kommt aus bereits geparsten TX-Instruction-Accounts / Cache, nicht aus `getAccount` im Sidefx.

### I-7 Nie RPC im Hot Path
Nie RPC in Hot Paths ohne explizite Freigabe — bricht Latenz-Anforderungen. Ausnahme nur hinter `allow_rpc_fallback == true` / `allow_rpc_on_miss == true` im Cold Path. Dieser PR darf keinen Cold-RPC-Seed an Pin oder TX-Apply haengen.

### I-9 Simulation-Gate
Wenn Simulation fehlschlaegt — nie senden (besonders Arbitrage). Dieser PR sendet keine TXs; nichts am Sim-Gate aendern.

### I-12 Decision Record
Jeder Intent endet mit Decision Record (Inputs, Checks, Outcome). Keine stille Ablehnung. **Keine erfundene v14-Liste** als Fake-Truth aus Pool-Account-Bytes.

### I-16 Geyser MASTER
Geyser MASTER in market-data + JetStream `POOL_CACHE` → SLAVE `LivePoolCache` ist autoritativ im Hot Path. RPC/WS nur Cold Path. Quotes bleiben Account-`ExecutableMarginal` in MASTER nach Pin. Das Adressbuch ist **keine** Quote-SSOT. C in JetStream-Metadata ist Layout, nicht Quote.

### I-MD-4 JetStream PoolCacheUpdate
`PoolCacheUpdate` fuer MASTER-LivePoolCache-Upserts. ExecHot darf nicht von JetStream abgekoppelt werden. Inhalt = gemergter MASTER. Wenn MASTER `pool_accounts` hat, darf die publizierte Nachricht sie nicht weglassen (sonst Last-Message-Bootstrap ohne C).

### I-MD-5 TX-Tracker ban
TX-Ingest enqueued kein `MdStateCommand::TrackMint` und erzeugt keinen unpinned `ExplicitConsumer::Tracker`-Explicit-Demand. Explicit-Geyser-Subscriptions entstehen nur via Wallet-Pin, Momentum Active Pools / Open-Position Pins, und Arb `track_requests`.

### I-MD-7 Admission Cap
Zu jedem Zeitpunkt `len(FixedCapAdmission) <= cap`. Owner-Gruppen atomar via `try_admit_owner_group`. Ungepinnt weiterhin 0 Subs. Adressbuch-Eintraege zaehlen **nicht** als Geyser-Subs. Buch-Cap 32768 bleibt hart.

### I-24d Cold-Path Discovery nur Request/Reply
execution-engine darf fehlende pool_accounts weder selbst discovern noch lokal in den SLAVE Cache schreiben. Dieser PR aendert den Control-Request-Pfad nicht und darf ihn nicht als Hot-Path-Ersatz missbrauchen.

### A.48 Quote-Vertrag (Ausschnitt)
Screening nur `ExecutableMarginal` aus Account-State. `LastTradeMid` / TX-as-quote verboten. Diesen PR nicht nutzen um `arb_max_leg_age_slots` oder `MAX_PRICE_AGE_MS` zu aendern.

### A.52 Adressbuch vs MASTER (dieser Task)
Ungepinnt: TX-Metas nur ins Adressbuch (Vault-/Bin-Pubkeys, Mints, Pump-`pool_accounts`). Kein MASTER-Upsert, kein Explicit-Zuwachs. Pin: Adressen nach MASTER kopieren, Buch-Eintrag loeschen, **dann** Rohr-B-Subscribe. Call-Order: Promote vor `try_admit_pool_consumer_group`. Promote darf Fill-Missing in eine vorhandene MASTER-Zeile machen. Nach Pin: TX nur fehlende Adressfelder in MASTER, nie Reserves/Quotes. Account-Decode = Fallback nach Pin. Kein RPC. Buch ist keine Quote-Quelle.

**Ergaenzung dieses PRs (kein Widerspruch zu TTL-Pflaster-Verbot):** Vollstaendige Pump-v14 im Buch sind Pin-Payload, nicht ungenutzter Discovery-Junk. Sie duerfen nicht an `DEFAULT_TTL_MS` sterben, bevor Promote sie nach MASTER kopiert. Die Konstante 120_000 bleibt fuer alle anderen Buch-Eintraege. Quote-TTLs bleiben unveraendert.

### Harte Trennung TX vs Account
TX = Discovery + Layout/C-Seed. Account = Quotes/Exits. Kein TX-Quote-Fallback. Spiegel zu #418: Account/Vault-Tick darf TX-`pool_accounts` nicht loeschen, auch nicht auf dem JetStream-Wire.

---

## Bestehendes Pattern (GitHub `architecture-rebuild` @ `4bc28c2`, nicht lokaler Clone)

**Ungepinnt → Buch** (`src/bin/market_data.rs` `apply_tx_pool_accounts_for_hot_pool`):

```rust
if !ctx.hot_pool_registry.is_hot_pool(pool) {
    if let Some(keys) = PoolLayoutKeys::from_cached_layout_state(&incoming) {
        book.merge(pool, keys, now);
        // metric AddressBook
    }
    return;
}
```

`cache_state_from_tx_pool_accounts` fuer `DexType::PumpFunAmm`: bei `accounts.len() >= 14` `pool_accounts = accounts.to_vec()`, sonst `Vec::new()`. Vaults aus `[4]/[5]`.

**Buch-TTL (das Loch):**

```rust
pub const DEFAULT_TTL_MS: u64 = 120_000;
pub fn evict_stale(&mut self, now_ms: u64) -> usize {
    self.entries.retain(|_, e| now_ms.saturating_sub(e.last_seen_ms) <= ttl);
}
```

`merge_layout_keys` Pump: `if ex_pa.is_empty() && !in_pa.is_empty() { incoming } else { existing }` — leere Incoming wipen C im Buch nicht; TTL wipet den ganzen Eintrag.

**Promote Fill-Missing** (`merge_tx_pool_accounts_into_existing`): dieselben Regeln fuer `pool_accounts`. `into_layout_only_cached_state` kopiert die Vec, Reserves `None`.

**Hot C-Write:** `set_pump_amm_pool_accounts_at_slot`; Pin-Seed-Metrik nur bei `Written`. `MissNoCacheEntry` → WARN + `write_miss{no_cache_entry}`. Prod write_miss=0.

**FIX-26 (Account-PoolDiscovered, behalten und spiegeln):**

```rust
let effective_pool_accounts = if !s.pool_accounts.is_empty() {
    s.pool_accounts.clone()
} else {
    host.live_pool_cache()
        .get_pump_amm_pool_accounts(pool_pubkey)
        .unwrap_or_default()
};
if !effective_pool_accounts.is_empty() {
    meta.insert("pool_accounts".to_string(), accounts_str.join(","));
}
```

**FIX-33 Trade-Publish:** `enqueue_jetstream(..., "FIX-33 pump_amm pool_accounts PoolCacheUpdate (trade)", false)` — behalten.

**SLAVE Preserve (#28):** `apply_pool_cache_update` kopiert existing `pool_accounts` wenn incoming leer. Reicht nicht fuer leeren Bootstrap.

**KNOWN_BUG_PATTERNS:** #28 PoolDiscovered wipe; #33 JetStream Bootstrap ohne C; #35 keine globale `protocol_fee_recipient`-Kanonisierung in diesem PR.

---

## Erlaubte Dateien

- `src/execution/pool_address_book.rs`
- `src/bin/market_data.rs` (Promote-Tests, ggf. duenne Wiring)
- `src/market_data/sidefx/handlers.rs` (BalanceUpdated-Metadata)
- `src/market_data/sidefx/host.rs` nur wenn Helper-Signatur noetig
- `src/execution/live_pool_cache.rs` nur Helper, kein Quote-Overwrite
- `src/execution/pool_cache_sync.rs` nur Test, wenn Last-Message-Apply bewiesen werden muss
- `src/metrics.rs` (optionale Counter)
- `docs/BUGS_FIXES.md`
- Tests neben den geaenderten Modulen
- Optional eine WARN-Zeile in `src/bin/arb_strategy.rs` bei `missing_accounts` — kein Gate-Relax, kein Refactor

---

## Verboten

- RPC in Sidefx / Pin / TX-Apply / `process_intent`
- Subscribe-all / TX-`TrackMint` / ungepinnt MASTER-Upsert
- Quote-/Reserve-Overwrite aus TX; LastTradeMid; TX-as-quote
- `DEFAULT_TTL_MS`, `MAX_PRICE_AGE_MS`, `arb_max_leg_age_slots` als Zahl anheben
- 14er-Gate in `create_arb_intent` lockern oder v14 aus Pool-Account erfinden
- Parser-Umbau Pump-Account-Count; DLMM-Quote-Fix; #35 Kanonisierung
- Eval-Repo in diesem PR
- Deploy; andere Branches als PR-gegen-`architecture-rebuild`

---

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --bin market-data
cargo test -p ironcrab pool_address_book
cargo test -p ironcrab --bin market-data promote
```

PR-Body: Pump v14 = Pin-Payload im Buch + JetStream-Last-Message; kein Subscribe-all; kein TTL-Zahlen-Bump; Verweis Findings 2026-09-15.
