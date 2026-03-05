# Docker MCP Toolkit – Schritt-für-Schritt Einrichtung

> **Hinweis (Stand 2026):** Der IronCrab-Supervisor nutzt jetzt **Open Brain** (Python-MCP-Server, PostgreSQL + pgvector) statt des Docker MCP Toolkits. Siehe `SETUP.md` und `OPENBRAIN_MCP_SPEC.md`. Dieses Dokument bleibt als Referenz für Nutzer, die Memory Reference (Docker MCP) weiterhin verwenden möchten.

**Zweck:** Memory-MCP-Tools nutzen, da `postgres-mcp-tools` auf Windows nicht zuverlässig funktioniert.

**Voraussetzung:** Docker Desktop 4.62+

---

## Schritt 1: MCP Toolkit aktivieren

1. **Docker Desktop** öffnen
2. **Einstellungen** (Zahnrad oben rechts) öffnen
3. Links **Beta features** wählen
4. **Enable Docker MCP Toolkit** aktivieren
5. **Apply & Restart** klicken (falls nötig)

---

## Schritt 2: Profil erstellen (optional)

Falls du mehrere MCP-Server organisieren möchtest:

1. In Docker Desktop links **MCP Toolkit** wählen (in der Sidebar)
2. Zum Tab **Profiles** wechseln
3. **Create** klicken
4. Profilname eingeben, z.B. **supervisor**
5. **Create profile** klicken

**Hinweis:** Die vorkonfigurierte `mcp.json` nutzt `--servers memory` direkt – ein Profil ist dafür nicht zwingend nötig.

---

## Schritt 3: Memory-Server verfügbar machen

Der Memory-Server ist Teil des Docker MCP Catalogs. Beim ersten Start von `docker mcp gateway run --servers memory` wird er bei Bedarf heruntergeladen.

**Optional (über Docker Desktop UI):**
1. Zum Tab **Catalog** wechseln
2. Nach **Memory** suchen
3. **Memory (Reference)** auswählen und zu einem Profil hinzufügen

**Hinweis:** Der Memory (Reference) Server nutzt ein Knowledge-Graph-Modell (Entities, Relations, Observations) – anders als postgres-mcp-tools, aber gut geeignet für Architektur-Entscheidungen und Failure-Patterns.

---

## Schritt 4: Cursor mit MCP Toolkit verbinden

### Option A: Über Cursor-Einstellungen (empfohlen)

1. **Cursor** öffnen
2. **Settings** → **Tools & MCP** (oder **Cursor Settings** → **MCP**)
3. Prüfen, ob **MCP_DOCKER** bereits erscheint (falls Docker Desktop Cursor automatisch verbunden hat)

### Option B: Manuell in mcp.json

Falls Cursor den MCP Toolkit nicht automatisch erkennt:

1. Datei `Trading_bot/.cursor/mcp.json` öffnen
2. Eintrag für MCP_DOCKER hinzufügen (siehe unten)

---

## Schritt 5: mcp.json anpassen

Die Datei `Trading_bot/.cursor/mcp.json` ist bereits angepasst. Der Eintrag lautet:

```json
{
  "mcpServers": {
    "MCP_DOCKER": {
      "command": "docker",
      "args": ["mcp", "gateway", "run", "--servers", "memory"]
    }
  }
}
```

**Alternative:** Falls du in Docker Desktop ein Profil mit mehreren Servern erstellt hast, nutze die CLI:
```bash
docker mcp client connect cursor
```
Dann wird Cursor automatisch mit dem Standard-Profil verbunden. Die manuelle mcp.json-Konfiguration mit `--servers memory` funktioniert unabhängig davon.

---

## Schritt 6: Cursor neu starten

1. Cursor vollständig schließen
2. Cursor erneut öffnen
3. Workspace `Trading_bot/` öffnen

---

## Schritt 7: Verbindung prüfen

1. Im Supervisor-Chat fragen: **„Welche MCP-Tools hast du?“**
2. Du solltest u.a. sehen: `create_entities`, `add_observations`, `search_nodes`, `read_graph` usw.
3. Test-Prompt: **„Erstelle eine Entity namens IronCrab mit der Observation: Rust-basierter Trading-Bot.“**

---

## Mapping: Supervisor-Memory → Memory (Reference)

| Supervisor (postgres-mcp-tools) | Memory (Reference) |
|--------------------------------|---------------------|
| add_memory (architectural_decision) | create_entities + add_observations |
| semantic_search | search_nodes |
| failure_pattern speichern | create_entities + add_observations |

**Beispiel für Architektur-Entscheidung:**
- Entity: `architectural_decision`
- Observation: `Hot-Path nutzt keine RPC-Calls für Latenz-Freiheit`
- Relation: `betrifft` → `IronCrab`

---

## Troubleshooting

### MCP_DOCKER erscheint nicht in Cursor / Verbindung startet nicht

**Häufige Ursache auf Windows:** Cursor startet mit anderem PATH als das Terminal – `docker` wird nicht gefunden.

**Lösung:** Vollen Pfad zu docker.exe in mcp.json verwenden:

```json
"command": "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe",
"args": ["mcp", "gateway", "run"]
```

(Ohne `--servers memory`, wenn du `docker mcp server enable memory` bereits ausgeführt hast.)

### MCP_DOCKER erscheint nicht in Cursor (weitere Checks)

- **Docker Desktop läuft?** Prüfen, ob Docker Desktop gestartet ist
- **Profil existiert?** In Docker Desktop → MCP Toolkit → Profiles prüfen
- **mcp.json korrekt?** Pfad zu `Trading_bot/.cursor/mcp.json` (Workspace-spezifisch) oder `~/.cursor/mcp.json` (global)

### docker mcp gateway run schlägt fehl

Im Terminal testen:

```powershell
docker mcp gateway run --profile supervisor
```

Falls Fehler: Docker Desktop neu starten, dann erneut versuchen.

### Keine Memory-Tools sichtbar

- Memory-Server dem Profil hinzugefügt? (MCP Toolkit → Profiles → Server prüfen)
- Cursor nach mcp.json-Änderung neu gestartet?
