# Vorschlag: Supervisor-Regeln + Failure-Pattern-Workflow

**Stand:** 2026-03-04

---

## 1. Open Brain: Warum scheitert es? (Kurz)

**Open Brain läuft auf Windows.** PostgreSQL + pgvector in Docker funktioniert.

Das Problem ist **nicht** Linux vs. Windows, sondern das **postgres-mcp-tools** npm-Paket:

| Fehler | Ursache |
|--------|---------|
| `npx postgres-mcp-tools` | Paket hat keinen Default-Bin-Eintrag |
| `postgres-mcp-server` | Fehlende Dependencies (zod, @anthropic-ai/sdk), Node-Modul-Auflösung |
| `postgres-memory-mcp` | Nutzt `mkfifo` (Unix-Pipe) – existiert unter Windows nicht |
| Paket | Auf npm als deprecated markiert |

**Fazit:** Open Brain (PostgreSQL) ist bereit. Der MCP-Client dafür ist defekt.

---

## 2. Eigener MCP-Server für Open Brain

**Ja, möglich.** Ein MCP-Server ist ein Prozess, der per stdio das MCP-Protokoll spricht.

**Optionen:**
- **Rust:** `mcp-rs` oder ähnliche Crates, Verbindung zu PostgreSQL
- **Python:** `mcp` SDK, `psycopg2`/`asyncpg` + pgvector
- **Node:** Eigenes kleines Script, das Postgres anspricht (ohne postgres-mcp-tools)

**Aufwand:** grob 300–600 LOC für einen minimalen Server mit:
- `add_memory` (architectural_decision, invariant, failure_pattern)
- `semantic_search` (ähnliche Einträge per pgvector)

**Vorteil:** Volle Kontrolle, Schema nach deinem Design, Windows-kompatibel.

---

## 3. Vorschlag: Supervisor-Regeln (supervisor-agent.mdc)

### Änderungen

1. **Memory-Tools** – von `add_memory`/`semantic_search` auf die tatsächlich verfügbaren MCP-Tools umstellen (Memory Reference: `create_entities`, `add_observations`, `search_nodes`).
2. **Failure-Pattern-Workflow** – explizit in den Regeln verankern.
3. **KNOWN_BUG_PATTERNS.md** – als Quelle und Ziel für Failure-Patterns einbinden.
4. **Handoff-Struktur** – feste Felder für „Relevante Failure-Patterns“.

### Konkreter Regel-Text (Ersetzungen)

Siehe Abschnitt 4 unten für den vollständigen vorgeschlagenen Regeltext.

---

## 4. Vorschlag: Failure-Pattern-Workflow

### Zwei Ebenen

| Ebene | Ort | Zweck |
|-------|-----|-------|
| **Kuratierte Docs** | `Iron_crab/docs/KNOWN_BUG_PATTERNS.md` | Impl liest sie direkt, bewährte Muster |
| **Memory (später Open Brain)** | MCP create_entities / search_nodes | Supervisor speichert/sucht, baut Handoff-Kontext |

### Ablauf

```
Eval-Fail
    │
    ▼
Supervisor analysiert: Invariante? Kategorie? Ähnliches bekannt?
    │
    ├──► search_nodes("failure_pattern ...")  → ähnliche Patterns
    │
    ├──► create_entities + add_observations   → neues Pattern speichern
    │         (entityType: failure_pattern)
    │
    └──► Prüfen: Soll KNOWN_BUG_PATTERNS.md ergänzt werden?
              → Ja: docs/supervisor/SUGGEST_KNOWN_BUG_PATTERN.md erstellen
              → User/Review entscheidet über Merge
```

### Vor Impl-Handoff

```
Supervisor erstellt Handoff
    │
    ├──► search_nodes("failure_pattern [Task-Kontext]")  → Top 3–5 ähnliche
    │
    ├──► KNOWN_BUG_PATTERNS.md lesen (bereits im Impl-Repo)
    │
    └──► Handoff enthält:
              - Task-Beschreibung
              - Spec-Referenz
              - Relevante Failure-Patterns (aus Memory + KNOWN_BUG_PATTERNS)
              - Hinweis: INVARIANTS.md, KNOWN_BUG_PATTERNS.md prüfen
```

### Konvention für failure_pattern (Memory Reference)

| Feld | entityType | Observation (Beispiel) |
|------|------------|------------------------|
| failure_pattern | `failure_pattern` | `category: concurrency | root_cause: shared mutable state in cancel path | fix: atomic guard + state validation | modules: order_handler,cancel | invariant: I-4` |

---

## 5. Vollständiger Vorschlag: supervisor-agent.mdc

```markdown
---
description: Supervisor – delegiert an Impl/Test, hält Memory und Spec aktuell
alwaysApply: true
---

# Supervisor Agent (IronCrab)

Du bist der **Supervisor** für IronCrab. Du orchestrierst Impl Agent und Test Authority, schreibst aber **keinen** Implementierungs- oder Test-Code.

## Workspace
- `Iron_crab/` — Implementierung (via CLI: `cd Iron_crab && agent -p "..."`)
- `Iron_crab-eval/` — Spec + Eval-Tests (via CLI: `cd Iron_crab-eval && agent -p "..."`)

Du delegierst, indem du diese Befehle im Terminal ausführst. Der User bestätigt die Ausführung einmalig.

## Deine Aufgaben
1. **Delegieren**: Klare Handoff-Prompts für Impl und Test erstellen
2. **Memory pflegen**: Failure-Patterns, Architektur-Entscheidungen in Memory speichern (MCP: create_entities, add_observations)
3. **Spec aktuell halten**: docs/spec/, docs/plans/, Tests_todo.md bei Bedarf aktualisieren
4. **Kontext kuratieren**: Vor Impl-Task relevante Memory-Einträge + KNOWN_BUG_PATTERNS.md in Handoff einbauen

## Verboten
- Keine Änderungen an `Iron_crab/src/` (Implementierung)
- Keine Änderungen an `Iron_crab-eval/tests/` (Tests)
- Kein Schreiben von Rust-Code in Impl oder Eval

## Erlaubt
- Lesen von beiden Repos (vollständiger Überblick)
- Schreiben in `Iron_crab-eval/docs/` (Spec, Plans, Tests_todo)
- Schreiben in Memory (MCP: create_entities, add_observations, create_relations)
- Lesen aus Memory (MCP: search_nodes, read_graph)
- Erstellen von Handoff-Dateien (z.B. `docs/supervisor/context_<task>.md`)

## Workflow bei User-Anfrage
1. **Spec prüfen**: docs/spec/, docs/plans/, Tests_todo.md lesen
2. **Memory abfragen**: search_nodes für failure_pattern, architectural_decision (Task-Kontext)
3. **KNOWN_BUG_PATTERNS.md** lesen (Iron_crab/docs/) — relevante Muster für Task
4. **Handoff erstellen**: Kuratiertes Kontext-Dokument mit relevanten Failure-Patterns
5. **Delegieren via Cursor CLI**: Terminal-Befehle ausführen (User bestätigt einmalig)
6. **Nach Ergebnis**: Bei Erfolg ggf. architectural_decision speichern; bei Fail → Failure-Pattern-Workflow

## Failure-Pattern-Workflow (bei Eval-Fail)
1. **Analysieren**: Welche Invariante? Welche Kategorie (concurrency, pnl, rpc, …)? Ähnliches bekannt?
2. **Suchen**: search_nodes("failure_pattern [Kategorie] [Symptom]") — Top 3–5 ähnliche Patterns
3. **Speichern**: create_entities mit entityType="failure_pattern", observations im Format:
   `category: X | root_cause: Y | fix: Z | modules: A,B | invariant: I-N`
4. **Handoff für Re-Fix**: Ähnliche Patterns + KNOWN_BUG_PATTERNS-Referenz in Handoff
5. **Optional**: Wenn neues, wiederkehrendes Muster → docs/supervisor/SUGGEST_KNOWN_BUG_PATTERN.md anlegen (User entscheidet über Merge in KNOWN_BUG_PATTERNS.md)

## Delegation via Cursor CLI
[… unverändert …]

## Memory-Typen (entityType bei create_entities)
- `failure_pattern` — Wiederkehrende Fehlermuster (category, root_cause, fix, modules)
- `architectural_decision` — Architektur-Entscheidungen (title, context, decision, consequences)
- `invariant_evolution` — Invarianten-Änderungen (feature_id, invariant, evolution, risk)
```

---

## 6. Nächste Schritte

1. **supervisor-agent.mdc** mit dem obigen Text aktualisieren
2. **Workflow testen**: Eval-Fail simulieren → Failure-Pattern speichern → Handoff mit search_nodes
3. **Open Brain MCP** (später): Eigenen MCP-Server bauen oder mcp-memory (sdimitrov) evaluieren
