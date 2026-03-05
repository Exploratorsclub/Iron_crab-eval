# Open Brain MCP — Nächste Schritte

**Status:** Setup abgeschlossen. Open Brain ist der Standard-Memory für den Supervisor (MCP_DOCKER entfernt).  
**Port:** 5433 (wegen möglicher Konflikte mit lokaler PostgreSQL auf 5432).

---

## 1. PostgreSQL starten

```powershell
cd c:\Users\Robert Onuk\Desktop\Trading_bot\openbrain
docker compose up -d
```

Falls die IronCrab-Tabellen noch fehlen (architectural_decisions, invariant_memory, failure_patterns):

```powershell
Get-Content "openbrain\migrations\02_ironcrab_schema.sql" | docker exec -i openbrain-postgres-1 psql -U memory_user -d memory_db
```

*(Container-Name ggf. anpassen: `docker ps`)*

---

## 2. Dependencies prüfen

```powershell
cd c:\Users\Robert Onuk\Desktop\Trading_bot\openbrain-mcp
pip install -r requirements.txt
```

Optional, für saubere Modulauflösung:

```powershell
pip install -e .
```

---

## 3. MCP-Server manuell testen

```powershell
cd c:\Users\Robert Onuk\Desktop\Trading_bot\openbrain-mcp
$env:POSTGRES_HOST="localhost"; $env:POSTGRES_PORT="5433"; $env:POSTGRES_USER="memory_user"; $env:POSTGRES_PASSWORD="memory_pass"; $env:POSTGRES_DB="memory_db"; $env:EMBEDDING_MODEL="mock"
python -m openbrain_mcp
```

Der Server blockiert auf stdio. Mit `Ctrl+C` beenden. Wenn er ohne Fehler startet, ist die DB-Verbindung ok.

---

## 4. Cursor neu starten

Die `mcp.json` enthält bereits den `openbrain`-Server. Nach Neustart von Cursor sollten die Tools sichtbar sein:

- `add_memory`
- `add_chat`
- `semantic_search`
- `list_recent`

---

## 5. In Cursor testen

Nach Neustart z.B. fragen: „Welche MCP-Tools hast du?“ oder direkt:

> Speichere einen Test-Eintrag mit add_memory: memory_type=architectural_decision, content="Test-Entscheidung für Open Brain".

---

## 6. Bei Fehlern

| Problem | Lösung |
|--------|--------|
| `python` nicht gefunden | In `mcp.json` `"command": "C:\\...\\python.exe"` mit vollem Pfad setzen |
| DB-Verbindung fehlgeschlagen | PostgreSQL läuft? `docker ps` prüfen |
| Tabelle fehlt | Migration `02_ironcrab_schema.sql` ausführen |
| Cursor sieht keine Tools | Cursor komplett neu starten, MCP-Log prüfen |

---

## 7. Supervisor-Regeln

Nach erfolgreichem Test: Supervisor nutzt `add_memory`, `add_chat`, `semantic_search`, `list_recent` statt Memory Reference für IronCrab-spezifisches Memory. Die Regeln in `supervisor-agent.mdc` verweisen bereits auf diese Tools.
