# Supervisor + Open Brain Setup

**Zweck:** Level-5 Workflow mit Supervisor-Agent, Engineering Memory und getrennter Impl/Test-Workspaces.

---

## 1. Workspace-Struktur

| Workspace | Ordner | Modell | Rolle |
|-----------|--------|--------|-------|
| **Supervisor** | `Trading_bot/` | Opus 4.6 | Delegiert, Memory, Spec |
| **Impl** | `Iron_crab/` | Composer 1.5 | Implementierung |
| **Eval** | `Iron_crab-eval/` | Composer 1.5 | Tests aus Spec |

### Einrichten

1. **Supervisor-Workspace öffnen**
   - Cursor → File → Open Folder → `Trading_bot/` wählen
   - Neues Fenster (oder Tab) öffnen
   - Modell: Opus 4.6 (Dropdown im Chat)

2. **Impl- und Eval-Agents** (via Cursor CLI)
   - Delegation: Supervisor führt `cd Iron_crab && agent -p "..."` bzw. `cd Iron_crab-eval && agent -p "..."` aus
   - CLI-Agent nutzt Composer 1.5 (oder konfiguriertes Modell)

---

## 2. Open Brain (Postgres + pgvector)

### Speicherbedarf

| Komponente | Größe |
|------------|-------|
| Docker-Image pgvector | ~250 MB |
| Leere DB + Schema | ~20 MB |
| Pro 1000 Memory-Einträge | ~2–5 MB |
| **Empfehlung frei** | **~1 GB** |

### Installation

```bash
cd Trading_bot/openbrain
docker compose up -d
```

Prüfen:

```bash
docker compose ps
# postgres sollte "healthy" sein
```

### Stoppen

```bash
docker compose down
```

---

## 3. MCP-Server (Open Brain)

Die MCP-Konfiguration liegt in `Trading_bot/.cursor/mcp.json` und wird automatisch geladen, wenn der Supervisor-Workspace (`Trading_bot/`) geöffnet ist.

### Voraussetzungen

- **Python** 3.10+ (mit `pip install -e .` in `openbrain-mcp/`)
- **Docker** mit laufendem Postgres (siehe oben, Port 5433)
- **openbrain-mcp** installiert: `cd openbrain-mcp && pip install -e .`

### Tools

| Tool | Zweck |
|------|-------|
| `add_memory` | failure_pattern, architectural_decision, invariant_evolution speichern |
| `add_chat` | Chat-/Konversations-Einträge speichern |
| `semantic_search` | Ähnliche Einträge finden (pgvector) |
| `list_recent` | Letzte N Einträge nach Typ |

### Embeddings

Aktuell: `EMBEDDING_MODEL=mock` — keine API-Keys nötig. Semantische Suche mit Mock ist eingeschränkt (andere Vektoren pro Text). Speichern und `list_recent` funktionieren zuverlässig.

Für bessere semantische Suche: `EMBEDDING_MODEL=openai` + `OPENAI_API_KEY` in `mcp.json` ergänzen.

### Test

1. Cursor neu starten (nach Änderungen an mcp.json)
2. Supervisor-Workspace öffnen
3. Im Chat: „Welche MCP-Tools hast du?“ oder „Speichere mit add_memory: memory_type=architectural_decision, content=Test-Entscheidung für I-4“

---

## 4. Ablauf

1. **Supervisor-Fenster** öffnen (Trading_bot/)
2. **Postgres starten**: `docker compose up -d` in `openbrain/`
3. **Task beschreiben**: z.B. „Implementiere I-4 laut plan_hot_path_rpc_freedom“
4. **Supervisor** erstellt Handoff, fragt Memory ab
5. **Supervisor führt aus**: `cd Iron_crab && agent -p "[Handoff]"` — User bestätigt die Ausführung
6. **CLI-Agent** arbeitet in Iron_crab, Ergebnis erscheint im Terminal
7. **Supervisor führt aus**: `cd Iron_crab-eval && cargo test` (oder delegiert Eval-Test)
8. Bei Fail: Fehler an Supervisor → failure_pattern speichern → neuer Handoff für Impl

---

## 5. Dateien

| Pfad | Zweck |
|------|-------|
| `Trading_bot/.cursor/rules/supervisor-agent.mdc` | Supervisor-Regel |
| `Trading_bot/.cursor/mcp.json` | MCP-Konfiguration |
| `Trading_bot/openbrain/docker-compose.yml` | Postgres-Container (Port 5433) |
| `Trading_bot/openbrain/init.sql` | DB-Schema (conversations) |
| `Trading_bot/openbrain/migrations/02_ironcrab_schema.sql` | IronCrab-Tabellen |
| `Trading_bot/openbrain-mcp/` | Python-MCP-Server |

---

## 6. Cursor CLI (für Delegation)

Die Cursor CLI ist **nicht** in der Cursor-IDE enthalten und muss **separat installiert** werden.

### Installation (Windows PowerShell)

```powershell
irm 'https://cursor.com/install?win32=true' | iex
```

### Prüfen

```powershell
agent --version
```

### Hinweis

Ohne CLI funktioniert die Delegation nicht automatisch. Du müsstest dann Handoffs manuell kopieren und in separate Cursor-Fenster (Iron_crab / Iron_crab-eval) einfügen.

---

## 7. Option 1 (CLI) vs Option 3 (Cloud Agents API)

| Kriterium | Option 1: Cursor CLI | Option 3: Cloud Agents API |
|-----------|----------------------|----------------------------|
| **Setup** | Ein Install-Befehl | API-Key, eigenes Script/Service |
| **Laufzeit** | Lokal, im Terminal | Cloud, remote |
| **Kosten** | Cursor-Abo (bereits vorhanden) | Zusätzlich API-Nutzung |
| **Trennung** | Per Verzeichnis (cd Iron_crab) | Pro Agent konfigurierbar |
| **Automatisierung** | Supervisor führt Befehl aus, User bestätigt | Vollautomatisch möglich |
| **Offline** | Ja (mit lokalem Modell) | Nein |
| **Komplexität** | Niedrig | Hoch (Orchestrierungs-Code) |

**Empfehlung:** Option 1 (CLI) für den Start. Option 3 nur, wenn du z.B. CI-Pipelines oder vollautomatische Runs ohne User-Interaktion brauchst.

---

## 8. Vollautomatische Delegation (Allowlist)

Mit der Cursor Command Allowlist kannst du die Supervisor-Delegation vollautomatisch machen – ohne manuelle Bestätigung der Terminal-Befehle.

### Einrichten

**Cursor Settings** → **Agents** → **Auto-Run** → **Command Allowlist**

Folgende Einträge hinzufügen (Präfix-Matching):

| Eintrag | Zweck |
|---------|-------|
| `cd Iron_crab && agent -p` | Impl-Delegation |
| `cd Iron_crab-eval && agent -p` | Eval-Delegation |
| `cd openbrain-mcp && agent -p` | Open-Brain-Delegation (selten) |
| `cd Iron_crab-eval && cargo test` | Tests ausführen |

### Alternative: Run Everything

**Auto-Run Mode** → **Run Everything** — alle Befehle laufen automatisch. Gilt global für alle Workspaces. Weniger gezielt, aber einfacher.

### Hinweis

Allowlist nutzt Präfix-Matching. Die gezielte Allowlist (Option A) ist sicherer, da nur Supervisor-Delegationsbefehle automatisch laufen.

---

## 9. Troubleshooting

### MCP-Server (Open Brain) startet nicht

- **Python prüfen**: `py -3 -c "import openbrain_mcp; print('OK')"`
- **Paket installiert**: `cd openbrain-mcp && pip install -e .`
- **Postgres läuft**: `docker compose ps` in openbrain/ (Port 5433)
- **Cursor neu starten** nach mcp.json-Änderungen

### Pfad mit Leerzeichen (z.B. "Robert Onuk")

Falls der Python-Pfad Leerzeichen enthält, nutzt `mcp.json` `C:\Windows\py.exe` statt `python.exe`. Das umgeht Pfad-Parsing-Probleme.

### semantic_search liefert keine Treffer (Mock-Embeddings)

Mit `EMBEDDING_MODEL=mock` erzeugt jeder Text andere Vektoren; semantische Ähnlichkeit ist eingeschränkt. Nutze `list_recent` für chronologische Abfragen. Für echte semantische Suche: `EMBEDDING_MODEL=openai` + `OPENAI_API_KEY` setzen.

### Embeddings: Von Mock zu OpenAI

Für bessere semantische Suche in mcp.json ergänzen:

```json
"EMBEDDING_MODEL": "openai",
"OPENAI_API_KEY": "sk-..."
```

OpenAI ada-002 ist sehr günstig (~$0.0001/1k Tokens).

---

## 10. Modelle in Cursor

- **Modell wechseln**: Chat-Panel → Dropdown oben
- **Opus 4.6**: Für Supervisor (Architektur, Kuratierung)
- **Composer 1.5**: Für Impl/Test (schnell, tägliche Arbeit)
