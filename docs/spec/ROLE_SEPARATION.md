# Role Separation & Least Privilege

**Stand:** 2026-08-22  
**Architektur:** `TARGET_ARCHITECTURE.md` (dieser Ordner). Onboarding: `CONTRIBUTING.md`.

Dieses Dokument beschreibt die **Sicherheitsarchitektur** und **Zugriffsrechte**.

## P0 Non-Negotiables

### Single Signer Prinzip
- **NUR `execution-engine`** hat Zugriff auf Wallet-Keys
- Alle anderen Prozesse sind **KEYLESS**: `market-data`, `momentum-bot`, `arb-strategy`, `position-manager`, `control-plane`, `trades-server`
- Verstoß = sofortiger Prozessabbruch (exit 1)

### Intent-Only Pattern
- Strategien (`momentum-bot`, `arb-strategy`) erzeugen ausschließlich `TradeIntent`s
- Keine direkten RPC/TPU/Jito Sends außerhalb der Execution Engine
- Intents über NATS/JetStream (`ironcrab.v1.trade_intents` / Stream `TRADE_INTENTS`)

## RBAC: Role-Based Access Control (Control Plane API)

### Rollen

| Rolle     | Beschreibung                      | Permissions                        |
|-----------|-----------------------------------|------------------------------------|
| `admin`   | Vollzugriff                       | read + write + kill switch         |
| `viewer`  | Nur-Lese-Zugriff                  | status, metrics, positions, logs   |
| `anonymous` | Dev-Mode ohne Auth             | Alle (nur wenn REQUIRE_AUTH=false) |

### Endpoint-Berechtigungen

| Endpoint                | Methode | Rolle erforderlich |
|-------------------------|---------|-------------------|
| `/health`               | GET     | (keine)           |
| `/rbac/info`            | GET     | (keine)           |
| `/whoami`               | GET     | viewer            |
| `/status`               | GET     | viewer            |
| `/positions`            | GET     | viewer            |
| `/metrics`              | GET     | viewer            |
| `/logs/{component}`     | GET     | viewer            |
| `/kill`                 | POST    | **admin**         |
| `/kill/reset`           | POST    | **admin**         |
| `/command/{component}`  | POST    | **admin**         |
| `/config`               | POST    | **admin**         |

### Konfiguration

```bash
# API Keys generieren
python -c "import secrets; print(secrets.token_urlsafe(32))"

# Environment Variables für Control Plane
CONTROL_PLANE_REQUIRE_AUTH=true       # Auth aktivieren (false = dev mode)
CONTROL_PLANE_ADMIN_KEY=<admin-key>   # Admin API Key
CONTROL_PLANE_VIEWER_KEY=<viewer-key> # Viewer API Key
```

### API-Nutzung

```bash
# Mit Admin-Key
curl -H "X-API-Key: $ADMIN_KEY" http://localhost:8080/kill -d '{"reason":"test"}'

# Mit Viewer-Key
curl -H "X-API-Key: $VIEWER_KEY" http://localhost:8080/status

# Ohne Auth (nur wenn REQUIRE_AUTH=false)
curl http://localhost:8080/status
```

### Audit-Logging

Alle authentifizierten Aktionen werden geloggt:
```
AUTH_SUCCESS: role=admin, key_prefix=abc12345
STATUS_VIEW: user=viewer, role=viewer
KILL_SWITCH_ACTIVATED: user=admin, reason='Manual stop', liquidate=True
```

## Prozess-Zugriffsmatrix

| Prozess           | Wallet Keys | NATS Publish          | NATS Subscribe        | Tx Sign/Send |
|-------------------|-------------|-----------------------|-----------------------|--------------|
| execution-engine  | ✅ JA       | ExecutionResults      | TradeIntents, Control | ✅ JA        |
| market-data       | ❌ NEIN     | MarketEvents, PoolCache MASTER | Control, Track-Requests | ❌ NEIN |
| momentum-bot      | ❌ NEIN     | TradeIntents          | MarketEvents, ExecResults | ❌ NEIN  |
| arb-strategy      | ❌ NEIN     | TradeIntents, Track-Requests | MarketEvents, PoolCache SLAVE | ❌ NEIN |
| position-manager  | ❌ NEIN     | Positions-KV          | Wallet snapshots, ExecResults | ❌ NEIN |
| control-plane     | ❌ NEIN     | Control Commands      | (Status Replies)      | ❌ NEIN      |
| trades-server     | ❌ NEIN     | —                     | JSONL / results       | ❌ NEIN      |

## Environment Variables

### execution-engine (einziger Key-Halter)
```bash
# Genau EINER dieser muss gesetzt sein:
IRONCRAB_KEYPAIR_JSON=     # JSON array [1,2,3,...] 32 oder 64 bytes
IRONCRAB_KEYPAIR_B64=      # Base64 encoded keypair
IRONCRAB_KEYPAIR_PATH=     # Pfad zu keypair.json
IRONCRAB_KEYPAIR_BASE58=   # Base58 encoded secret

# Optional für strengere Pfad-Validierung:
IRONCRAB_KEYPAIR_STRICT=1
IRONCRAB_KEYPAIR_ALLOWED_DIRS=/home/ironcrab/.config/solana
```

### Alle anderen Prozesse
```bash
# KEINE der obigen KEYPAIR Variablen setzen!
# Prozess crasht mit exit(1) wenn Keys erkannt werden.
```

## Enforcement im Code

### market-data (src/bin/market_data.rs)
```rust
if std::env::var("IRONCRAB_KEYPAIR_JSON").is_ok()
    || std::env::var("IRONCRAB_KEYPAIR_B64").is_ok()
    || std::env::var("IRONCRAB_KEYPAIR_PATH").is_ok()
{
    error!("market-data is KEYLESS per architecture");
    std::process::exit(1);
}
```

### momentum-bot, arb-strategy, position-manager
Identische Keypair-Prüfung mit `exit(1)` (Rust). `position-manager` ist keyless KV-Writer (`IRONCRAB_WALLET_PUBKEY` ist die **Pubkey**, nicht der Secret).

### control-plane (Python)
```python
# Prüfung beim Startup
forbidden_vars = ["IRONCRAB_KEYPAIR_JSON", "IRONCRAB_KEYPAIR_B64", 
                  "IRONCRAB_KEYPAIR_PATH", "IRONCRAB_KEYPAIR_BASE58"]
if any(os.getenv(v) for v in forbidden_vars):
    raise RuntimeError("Control Plane cannot start with wallet keys")
```

## NATS ACL Konfiguration (Production)

Für Production sollte NATS mit ACLs konfiguriert werden:

```hcl
# /etc/nats/nats.conf

authorization {
  users = [
    # execution-engine: Kann Intents empfangen, Results senden
    {
      user: "execution-engine"
      password: "$EXEC_NATS_PASS"
      permissions: {
        subscribe: ["ironcrab.v1.trade_intents", "ironcrab.v1.control.>"]
        publish: ["ironcrab.v1.execution_results", "_INBOX.>"]
      }
    }
    
    # market-data: Kann nur Events publizieren
    {
      user: "market-data"
      password: "$MD_NATS_PASS"
      permissions: {
        subscribe: []
        publish: ["ironcrab.v1.market_events", "ironcrab.pool_cache.*"]
      }
    }
    
    # momentum-bot: Empfängt Events, sendet Intents
    {
      user: "momentum-bot"
      password: "$MB_NATS_PASS"
      permissions: {
        subscribe: ["ironcrab.v1.market_events"]
        publish: ["ironcrab.v1.trade_intents"]
      }
    }
    
    # arb-strategy: Empfängt Events/Cache, sendet Intents (keyless)
    {
      user: "arb-strategy"
      password: "$ARB_NATS_PASS"
      permissions: {
        subscribe: ["ironcrab.v1.market_events", "ironcrab.pool_cache.*"]
        publish: ["ironcrab.v1.trade_intents"]
      }
    }

    # control-plane: Control Commands, keine Trading Topics
    {
      user: "control-plane"
      password: "$CP_NATS_PASS"
      permissions: {
        subscribe: ["_INBOX.>"]
        publish: ["ironcrab.v1.control.>"]
      }
    }
  ]
}
```

## Audit Logging

### Control Plane Audit Log
Alle administrativen Aktionen werden in `control_plane_audit.log` geloggt:

```
2024-12-30 10:15:00 - AUDIT - STARTUP: Control Plane started (keyless mode verified)
2024-12-30 10:20:00 - AUDIT - COMMAND: component=momentum-bot, command=pause, params={}
2024-12-30 10:25:00 - AUDIT - CONFIG_UPDATE: component=execution-engine, keys=['max_position_sol']
2024-12-30 11:00:00 - AUDIT - KILL_SWITCH_ACTIVATED: reason='Manual intervention', liquidate=True
```

### Decision Records (execution-engine)
Jede Trade-Entscheidung wird in `trade_logs/decisions/` aufgezeichnet:
- Input-Snapshot (Intent + Marktdaten)
- Reason Code (ACCEPTED, REJECTED_*)
- Outcome (Signature oder Error)

## Systemd Hardening

### Keypair File Permissions
```bash
# Nur ironcrab User kann lesen
chmod 600 /home/ironcrab/.config/solana/id.json
chown ironcrab:ironcrab /home/ironcrab/.config/solana/id.json
```

### Service-spezifische Umgebung
```ini
# execution-engine.service
Environment=IRONCRAB_KEYPAIR_PATH=/home/ironcrab/.config/solana/id.json

# momentum-bot.service / arb-strategy.service / position-manager.service / market-data.service
# KEINE KEYPAIR Variable!
# position-manager: IRONCRAB_WALLET_PUBKEY (Pubkey only) ist erlaubt.
```

### Zusätzliche Hardening-Optionen
```ini
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=read-only
```

## Verifikation

### Test: Key-Isolation prüfen
```bash
# Auf dem Server: Prüfen dass nur execution-engine Keys hat
sudo -u ironcrab printenv | grep KEYPAIR
# Sollte nur in execution-engine Kontext erscheinen

# Prüfen dass momentum-bot ohne Keys startet
systemctl status momentum-bot
# Log sollte KEINE Keypair-Warnungen zeigen
```

### Test: NATS ACL prüfen (wenn konfiguriert)
```bash
# Mit momentum-bot Credentials sollte publish auf intents.> funktionieren
nats pub ironcrab.v1.trade_intents "test" --user momentum-bot --password $MB_PASS
# Aber NICHT auf control.>
nats pub ironcrab.v1.control.test "test" --user momentum-bot --password $MB_PASS
# Expected: Permissions Violation
```

## Checkliste vor Go-Live

- [ ] Keypair-Datei nur für ironcrab User lesbar (chmod 600)
- [ ] Nur execution-engine.service hat KEYPAIR Environment Variable
- [ ] market-data, momentum-bot, arb-strategy, position-manager crashen mit exit(1) wenn Secrets erkannt
- [ ] control-plane crasht beim Start wenn Keys erkannt
- [ ] Audit-Log aktiviert und rotiert
- [ ] (Optional) NATS ACLs konfiguriert
- [ ] Decision Records werden geschrieben
