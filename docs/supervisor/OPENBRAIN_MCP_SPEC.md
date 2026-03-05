# Open Brain MCP Server — Spec (IronCrab-optimiert)

**Zweck:** Python-MCP-Server für den IronCrab Supervisor. Verbindet Cursor mit Open Brain (PostgreSQL + pgvector). Open Brain dient als **zentraler Speicher für alles Projektrelevante**: Chat-Verlauf, Failure-Patterns, Architektur-Entscheidungen, Invarianten-Evolution.

**Kontext:** IronCrab ist ein Solana-Trading-Bot. Der Supervisor orchestriert Impl- und Eval-Agents. Memory dient der Qualitätssteigerung und Nachvollziehbarkeit: wiederkehrende Fehler vermeiden, Architektur-Kohärenz, Invarianten-Kontinuität, vollständiger Projekt-Kontext.

---

## 1. Voraussetzungen

- **PostgreSQL** mit pgvector (openbrain/ läuft via `docker compose up -d`)
- **Python** 3.10+
- **Verbindung:** localhost:5433 (Port 5433 wegen möglicher Konflikte mit lokaler PostgreSQL), User: memory_user, DB: memory_db, Passwort: memory_pass

---

## 2. MCP-Tools (Übersicht)

| Tool | Zweck |
|------|--------|
| `add_memory` | Strukturierte Einträge (failure_pattern, architectural_decision, invariant_evolution) |
| `add_chat` | Chat-/Konversations-Einträge (Supervisor-Dialoge, Delegationen) |
| `semantic_search` | Ähnliche Einträge finden (alle Typen oder gefiltert) |
| `list_recent` | Letzte N Einträge nach Typ (für Kontext, Überblick) |

---

### 2.1 add_memory

Speichert einen Memory-Eintrag. Erstellt Embedding und schreibt in die passende Tabelle.

**Parameter:**

| Name | Typ | Pflicht | Beschreibung |
|------|-----|---------|--------------|
| `memory_type` | string | ja | `failure_pattern` \| `architectural_decision` \| `invariant_evolution` |
| `content` | string | ja | Der zu speichernde Text (wird für Embedding und Suche genutzt) |
| `metadata` | object | nein | Zusätzliche Felder je nach Typ |

**metadata je nach memory_type:**

**failure_pattern:**
```json
{
  "category": "concurrency|pnl|rpc|position-tracking|dex|lifecycle|...",
  "root_cause": "Kurzbeschreibung der Ursache",
  "fix": "Kurzbeschreibung des Fixes",
  "modules": ["order_handler", "pool_cache", ...],
  "invariant": "I-4",
  "symptoms": "Optionale Symptome"
}
```

**architectural_decision:**
```json
{
  "title": "Kurzer Titel",
  "context": "Warum die Entscheidung nötig war",
  "decision": "Die Entscheidung",
  "consequences": "Folgen",
  "tags": ["hot_path", "matching", "determinism"]
}
```

**invariant_evolution:**
```json
{
  "feature_id": "I-4",
  "invariant_text": "Der invariante Text",
  "evolution_note": "Was sich geändert hat",
  "risk_category": "state-transition|concurrency|..."
}
```

**Rückgabe:** `{ "id": "uuid", "memory_type": "...", "created_at": "..." }`

---

### 2.2 semantic_search

Sucht semantisch ähnliche Einträge über alle Speichertypen. Nutzt pgvector cosine similarity.

**Parameter:**

| Name | Typ | Pflicht | Beschreibung |
|------|-----|---------|--------------|
| `query` | string | ja | Suchanfrage (wird für Embedding genutzt) |
| `memory_type` | string | nein | Filter: `failure_pattern` \| `architectural_decision` \| `invariant_evolution` \| `chat`. Ohne Angabe: Suche über alle Typen |
| `limit` | int | nein | Max. Anzahl Treffer (Default: 5) |

**Tabellen-Mapping:** `failure_pattern` → failure_patterns, `architectural_decision` → architectural_decisions, `invariant_evolution` → invariant_memory, `chat` → conversations.

**Rückgabe:** `{ "results": [ { "id", "memory_type", "content", "metadata", "similarity" }, ... ] }`

---

### 2.3 add_chat

Speichert einen Chat-/Konversations-Eintrag. Schreibt in `memory.conversations`. Erstellt Embedding für spätere semantische Suche.

**Parameter:**

| Name | Typ | Pflicht | Beschreibung |
|------|-----|---------|--------------|
| `conversation_id` | string | nein | Session-/Konversations-ID (gruppiert zusammengehörige Nachrichten) |
| `role` | string | ja | `user` \| `assistant` \| `system` |
| `content` | string | ja | Der Nachrichtentext |
| `metadata` | object | nein | Z.B. `{"task_ref": "I-4", "delegation_target": "impl"}` |

**Rückgabe:** `{ "id": "uuid", "conversation_id": "...", "created_at": "..." }`

---

### 2.4 list_recent

Liefert die letzten N Einträge, optional gefiltert nach Typ.

**Parameter:**

| Name | Typ | Pflicht | Beschreibung |
|------|-----|---------|--------------|
| `memory_type` | string | nein | Filter: `failure_pattern` \| `architectural_decision` \| `invariant_evolution` \| `chat` |
| `limit` | int | nein | Max. Anzahl (Default: 10) |

**Rückgabe:** `{ "results": [ { "id", "memory_type", "content", "metadata", "created_at" }, ... ] }`

---

## 2.5 Speichertypen-Übersicht

| Typ | Tabelle | Tool zum Schreiben | Inhalt |
|-----|---------|-------------------|--------|
| `chat` | memory.conversations | add_chat | Supervisor-Dialoge, Delegationen, Entscheidungen |
| `failure_pattern` | memory.failure_patterns | add_memory | Wiederkehrende Bug-Muster |
| `architectural_decision` | memory.architectural_decisions | add_memory | Architektur-Entscheidungen |
| `invariant_evolution` | memory.invariant_memory | add_memory | Invarianten-Änderungen |

---

## 3. Datenbankschema

**Schema:** `memory`

### 3.0 memory.conversations (Chat-Verlauf)

Bereits in `openbrain/init.sql` vorhanden. Wird von `add_chat` genutzt.

| Spalte | Typ | Beschreibung |
|--------|-----|--------------|
| id | SERIAL | PK |
| conversation_id | TEXT | Session-ID (z.B. Cursor-Session) |
| user_id | TEXT | Optional |
| timestamp | TIMESTAMPTZ | Erstellzeit |
| content | TEXT | Nachrichtentext |
| embedding | vector(1536) | Für semantic_search |
| metadata | JSONB | role, task_ref, delegation_target, etc. |
| is_archived | BOOLEAN | Optional archiviert |
| last_accessed | TIMESTAMPTZ | Letzter Zugriff |

**Konvention:** `metadata->>'role'` = `user` | `assistant` | `system`

---

Zusätzlich drei Tabellen für strukturierte IronCrab-Einträge:

### 3.1 memory.architectural_decisions

```sql
CREATE TABLE IF NOT EXISTS memory.architectural_decisions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  title TEXT NOT NULL,
  context TEXT,
  decision TEXT NOT NULL,
  consequences TEXT,
  tags TEXT[],
  embedding vector(1536),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_arch_decisions_embedding
  ON memory.architectural_decisions USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX IF NOT EXISTS idx_arch_decisions_tags ON memory.architectural_decisions USING GIN (tags);
```

### 3.2 memory.invariant_memory

```sql
CREATE TABLE IF NOT EXISTS memory.invariant_memory (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  feature_id TEXT NOT NULL,
  invariant_text TEXT NOT NULL,
  evolution_note TEXT,
  risk_category TEXT,
  embedding vector(1536),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_invariant_embedding
  ON memory.invariant_memory USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX IF NOT EXISTS idx_invariant_feature ON memory.invariant_memory (feature_id);
```

### 3.3 memory.failure_patterns

```sql
CREATE TABLE IF NOT EXISTS memory.failure_patterns (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  category TEXT NOT NULL,
  root_cause TEXT NOT NULL,
  symptoms TEXT,
  fix_strategy TEXT,
  related_modules TEXT[],
  embedding vector(1536),
  frequency INT DEFAULT 1,
  last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_failure_patterns_embedding
  ON memory.failure_patterns USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX IF NOT EXISTS idx_failure_patterns_category ON memory.failure_patterns (category);
CREATE INDEX IF NOT EXISTS idx_failure_patterns_frequency ON memory.failure_patterns (frequency DESC);
```

**Migration:** `openbrain/migrations/02_ironcrab_schema.sql`

Bei bestehender Datenbank ausführen (aus Trading_bot/):
```powershell
Get-Content openbrain\migrations\02_ironcrab_schema.sql | docker exec -i openbrain-postgres-1 psql -U memory_user -d memory_db
```

---

## 4. Embeddings

**Dimension:** 1536 (OpenAI text-embedding-3-small / ada-002)

**Empfohlene Strategie:**
- **Entwicklung:** `EMBEDDING_MODEL=mock` — zufällige Vektoren, keine API. Semantische Suche eingeschränkt, aber Speichern/Abrufen funktioniert.
- **Produktion:** `EMBEDDING_MODEL=openai` + `OPENAI_API_KEY` — echte semantische Suche.

**Zu embeden:** `content` (bei add_memory) bzw. `query` (bei semantic_search). Nicht ganze Dateien, nur die granularen Texte.

---

## 5. IronCrab-spezifische Konventionen

### 5.1 failure_pattern Kategorien

Angelehnt an `Iron_crab/docs/KNOWN_BUG_PATTERNS.md`:

| Kategorie | Beispiele |
|-----------|-----------|
| `pnl` | Wrong-Pool Price, fill_in/fill_out, invertierte PnL-Formel (Pattern 1–3, 11) |
| `rpc` | RPC im Hot Path (Pattern 4) |
| `position-tracking` | Ghost Positions, Orphaned Buy (Pattern 5–6) |
| `concurrency` | exit_generated, Doppelter JetStream Consumer, LockManager (Pattern 7–8, 15) |
| `dex` | Hardcoded DEX-Namen, pool_accounts Count, Account Order (Pattern 9, 14, 20) |
| `lifecycle` | WSOL, Token-2022, Liquidation (Pattern 16–18) |
| `fix-revert` | Falsche Root Cause (Pattern 19) |

### 5.2 Module-Namen (related_modules)

Beispiele aus IronCrab: `order_handler`, `pool_cache`, `lock_manager`, `wsol_manager`, `tx_builder`, `execution_result`, `jetstream`, `liquidation`.

### 5.3 Feature-IDs

Invarianten: `I-4`, `I-5`, etc. (aus docs/plans/, Tests_todo.md).

### 5.4 Tags (architectural_decisions)

Beispiele: `hot_path`, `cold_path`, `matching`, `determinism`, `rpc_freedom`, `geyser`, `pool_cache`.

### 5.5 Chat-Metadaten (add_chat)

Für bessere Nachvollziehbarkeit in `metadata`:
- `task_ref`: Referenz auf Task (z.B. I-4)
- `delegation_target`: `impl` | `eval` | `none`
- `session_id`: Cursor-Session für Gruppierung

---

## 6. Umgebungsvariablen

| Variable | Pflicht | Default | Beschreibung |
|----------|---------|---------|--------------|
| `POSTGRES_HOST` | nein | localhost | PostgreSQL Host |
| `POSTGRES_PORT` | nein | 5432 | Port |
| `POSTGRES_USER` | nein | memory_user | User |
| `POSTGRES_PASSWORD` | ja | — | Passwort |
| `POSTGRES_DB` | nein | memory_db | Datenbank |
| `EMBEDDING_MODEL` | nein | mock | `mock` oder `openai` |
| `OPENAI_API_KEY` | bei openai | — | API-Key |

---

## 7. Technische Anforderungen

- **Transport:** stdio (MCP-Standard)
- **Sprache:** Python 3.10+
- **Abhängigkeiten:** `mcp`, `asyncpg`, `pgvector`, ggf. `openai`
- **Struktur:** Einzelnes `server.py` oder kleines Modul, minimaler Aufwand

---

## 8. Integration mit Cursor

Nach Implementierung in `Trading_bot/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "openbrain": {
      "command": "python",
      "args": ["-m", "openbrain_mcp"],
      "cwd": "Trading_bot/openbrain-mcp",
      "env": {
        "POSTGRES_HOST": "localhost",
        "POSTGRES_PORT": "5433",
        "POSTGRES_USER": "memory_user",
        "POSTGRES_PASSWORD": "memory_pass",
        "POSTGRES_DB": "memory_db",
        "EMBEDDING_MODEL": "mock"
      }
    }
  }
}
```

Oder mit vollem Pfad zu `python.exe` (analog zu docker.exe bei Windows).

---

## 9. Mapping: Supervisor-Regeln → MCP-Tools

| Supervisor-Aktion | MCP-Tool | Parameter |
|-------------------|----------|-----------|
| Failure-Pattern nach Eval-Fail speichern | add_memory | memory_type: failure_pattern, content: Zusammenfassung, metadata: category, root_cause, fix, modules, invariant |
| Ähnliche Patterns vor Impl suchen | semantic_search | query: "[Kategorie] [Symptom]", memory_type: failure_pattern, limit: 5 |
| Architektur-Entscheidung speichern | add_memory | memory_type: architectural_decision, content + metadata |
| Relevante Architektur-Constraints vor Impl | semantic_search | query: "[Modul] [Constraint]", memory_type: architectural_decision, limit: 5 |
| Invarianten-Evolution | add_memory | memory_type: invariant_evolution |
| Chat-Nachricht speichern | add_chat | role, content, conversation_id, metadata |
| Ähnliche Chat-Einträge / Kontext suchen | semantic_search | query: "[Thema]", memory_type: chat, limit: 5 |
| Letzte Einträge für Überblick | list_recent | memory_type: optional, limit: 10 |

---

## 10. Abnahmekriterien

- [ ] Server startet mit `python -m openbrain_mcp` (oder `python server.py`)
- [ ] add_memory speichert in die richtige Tabelle
- [ ] add_chat speichert in memory.conversations
- [ ] semantic_search liefert Ergebnisse (alle Typen inkl. chat)
- [ ] list_recent liefert Einträge nach Typ
- [ ] Cursor erkennt die Tools nach Neustart
- [ ] Supervisor kann failure_pattern speichern und vor Handoff abrufen
- [ ] Chat-Verlauf wird bei Bedarf gespeichert und abrufbar
