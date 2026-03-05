# Handoff: Open Brain MCP Server implementieren

**Ziel:** Python-MCP-Server für Open Brain vollständig implementieren.

**Spec:** `Iron_crab-eval/docs/supervisor/OPENBRAIN_MCP_SPEC.md` (im Projekt-Root: `../Iron_crab-eval/docs/supervisor/OPENBRAIN_MCP_SPEC.md`)

**Kontext:** Du arbeitest im Ordner `openbrain-mcp/`. Das Grundgerüst (server.py, requirements.txt) ist vorhanden. Die Datenbank ist vorbereitet (Migration ausgeführt).

## Aufgabe

1. **Spec lesen** — OPENBRAIN_MCP_SPEC.md vollständig durchgehen
2. **MCP-Server implementieren** — mit stdio-Transport, Tools: add_memory, add_chat, semantic_search, list_recent
3. **PostgreSQL + pgvector** — asyncpg, pgvector für Embeddings und Similarity-Search
4. **Embeddings** — mock (zufällig/deterministisch) für Entwicklung; optional OpenAI-Integration
5. **Testen** — Server startet, Tools können aufgerufen werden

## Erlaubte Dateien

- `openbrain-mcp/*` — alles in diesem Ordner

## Hinweise

- PostgreSQL: localhost:5432, User: memory_user, DB: memory_db, Passwort: memory_pass
- Tabellen: memory.architectural_decisions, memory.invariant_memory, memory.failure_patterns, memory.conversations
- MCP Python SDK: `mcp` (pip install mcp)
- Embedding-Dimension: 1536

## Abnahmekriterien (aus Spec)

- [ ] Server startet mit `python server.py` oder `python -m openbrain_mcp`
- [ ] add_memory speichert in die richtige Tabelle
- [ ] add_chat speichert in memory.conversations
- [ ] semantic_search liefert Ergebnisse
- [ ] list_recent liefert Einträge nach Typ
