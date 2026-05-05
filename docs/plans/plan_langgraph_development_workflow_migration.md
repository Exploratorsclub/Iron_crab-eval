# Plan: Wechsel auf LangGraph fuer den lokalen Entwicklungsworkflow

**Status:** Phasen 1–7 und der **implementierte** LangGraph‑Orchestrator (**`Langgraph_workflow`**, Hybrid Level 5 etc.) dokumentiert ✅. **„Phase 8/9“** im alten Sinne („Parallelbetrieb / Abbau Cursor“) entfällt logisch siehe Abschnitt *Adoption*: Der Graph **startet keine** Cursor Cloud Agents; sobald ihr LangGraph fuer die Arbeit nutzt und **Cursor Cloud fuer dieselbe Arbeit nicht mehr anwerft**, endet Cursor‑Orchestrierung von selbst — **Phase 9** als separates Rollout‑Kapitel ist damit ueberfluessig. **⏳** verbleiben nur dokumentarisch: ideale Knotennamen (**`write_handoff`** vs **`generate_handoff`**).  
**Datum:** 2026-05-02 (Stand Umsetzung / Indexierung: 2026-05-)  
**Betroffene Repos:** Separate GitHub-Repos **`Iron_crab`** (Implementierung, Rust) und **`Iron_crab-eval`** (Spec, Supervisor-Doku, Eval-Tests); lokal typisch **`C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab`** und **`C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab-eval`** im selben Ursprungsordner (oft der **gleiche Ordnerbaum wie dein lokaler Workspace in der Cursor IDE** unter `Trading_bot`). Der Python-**Orchestrator** liegt **ausserhalb** dieser Repo-Roots unter **`C:\Users\Robert Onuk\Langgraph_workflow`** und ist **davon zur Laufzeit unabhängig** (kein pip- oder Runtime-Dependency auf Cursor).  
**Motivation:** Die bisherige Orchestrierung über **Cursor Cloud Agents** soll durch einen vollständig **lokalen** LangGraph-Workflow ersetzt werden. **`Langgraph_workflow`** importiert keine Cursor-/Cloud-Agent-Clients und enthält keine Graph-Knoten für proprietäre Review-Bots (**Bugbot** o.Ä. können bei Bedarf **manuell** auf GitHub laufen — **orthogonal** zum Graphen, nicht eingebunden). Der Trading-Bot selbst bleibt in Rust; LangGraph übernimmt Supervisor-, Impl-, Eval- und Review-Schritte über lokal dokumentierbare Mittel (`gh`, SSH-Debug mit Blockliste, Open Brain optional).

### Legende: Umsetzung vs. Plan (Orchestrator `Langgraph_workflow`)

| Symbol | Bedeutung |
|--------|-----------|
| **✅** | Im Orchestrator umgesetzt |
| **⏳** | Teilweise / anders strukturiert (funktional aehnlich) |
| **📋** | Noch offen |

**High-Level:** ✅ Phasen 1–7; ✅ **implementierter** Graph (LM-Supervisor, `run_code_reviewer_lm`, Hybrid `IRONCRAB_HYBRID_LEVEL5`, `generate_handoff`, CI, OpenBrain, …). ✅ **Code-Indexierung**, **`gather_context`** (Specs, Branches, `pr_url`), **SSH‑Debug**. ⏳ Zielgraph-Namen in der Planskizze (`write_handoff` vs **`generate_handoff`** — im Code bereits **generate_handoff**). Adoption: **ein paar echte Scopes mit LangGraph** zum Vertrauensaufbau bleiben sinnvoll, sind aber **kein** zweites Projektphasen‑Gate (siehe *Adoption* unten): **„Phase 8 Parallel + Phase 9 Abbau“** ist logisch ueberfluessig, weil der Orchestrator Cursor Cloud ohnehin **nie** ausloesen kann.

### Phase-0 — getroffene Entscheidungen

| Thema | Entscheidung |
|-------|----------------|
| Ablage Orchestrator | **`C:\Users\Robert Onuk\Langgraph_workflow`** — **kein zweiter Git-Checkout** fuer Impl/Eval **in diesem Ordner**; die Repos liegen getrennt (bei dir **`Desktop\Trading_bot\...`**). Eigenes Repo fuer den Orchestrator ist **nicht** vorgesehen. |
| GitHub | **Weiter fuer Versionskontrolle und Deploy** (Remote, Branching, nach Bedarf Actions/CI). |
| PR vs. Direktpush | **Sinnvolle Mischung:** **Alle Impl- und Eval-Code-/Test-Aenderungen** laufen ueber **Pull Request**. **Direktpush nur** fuer **Doku ohne Build-/CI-Auswirkung**, oder fuer Arbeit **ausdruecklich auf einem geschuetzten Feature-Branch**, den du **spaeter per PR** in den Ziel-Branch zusammenfuehrst — **nie** Direktpush in geschuetzte Hauptlinien ohne PR, wenn Impl/Eval betroffen ist. |
| Trennung Impl/Eval | Auf GitHub **zwei Repos**; lokal zwei **Git-Roots** (**Default:** `Trading_bot\Iron_crab` und `Trading_bot\Iron_crab-eval` unter **`C:\Users\Robert Onuk\Desktop\Trading_bot`**). **Isolation:** zwei getrennte `.git`-Verzeichnisse, **repo_paths**/Tools pro Rolle — der gemeinsame Parent `Trading_bot` ist nur Ordnerlayout, keine Aufhebung von Level 5 im Git-Sinne. |
| Supervisor / Server | Supervisor darf sich per **`ssh ironcrab-prod`** (laut Server-Doku/SSH-Konfig) mit dem Produktionsserver verbinden fuer **Debugging** (Logs, Status, fuer Diagnose erforderliche Befehle). **Automatisches Deploy ueber SSH ist verboten** — Deploy nur manuell durch den User siehe Produktionsschutz. |
| Code-Indexierung | Kein automatisches **Cursor IDE**-Repo-weites Indexing wie in der IDE; explizites **Indexing/RAG getrennt pro Repo**; Agent-Tools sind an den jeweiligen Index gebunden; Supervisor darf **beide Indexes lesen** oder optional einen **Supervisor-/Kontext-Index** mit aggregiertem Doku-/Spec-Kontext nutzen — siehe *Code-Indexierung und Retrieval*. |
| LM Studio Chat (Alle Agenten) | **Festgelegt:** OpenAI-kompatible Base URL **`http://100.106.158.144:1234/v1`** (Netbird zur AI-Maschine), Modell **`qwen3-coder-next`**. Zugriff getestet. **Alle Rollen** (Supervisor, Impl, Eval, Reviewer) nutzen vorerst **dieselbe** selbst gehostete Qwen-/LM-Studio-Anbindung. |
| Embedding fuer Indizes (RAG) | **Festgelegt:** **Option D (Hybrid):** Embedding und **persistente Indizes primaer auf dem Orchestrator-PC** unter `Langgraph_workflow\data\index_*` — **LM-Studio-Rechner nur fuer Chat-/Completion**, keine Embedding-Workload dort. Umsetzung: **Python** (z.B. **sentence-transformers** mit GPU-Unterstuetzung auf dem Orchestrator) als Teil von `index_builder`. **Hinweis zum Test mit `curl`:** OpenAI-kompatibel ist Embeddings typischerweise **POST** mit JSON-Body; ein **GET** auf `/v1/embeddings` liefert oft „Unexpected method“ — zudem muss die LM-Studio-Instanz Embeddings **ueberhaupt** anbieten. Da die AI-Maschine **nur Chat** betreiben soll, entfaellt LM-Studio-Embedding als Pflicht. **Default-Vorschlag (Embedding-Checkpoint):** **`BAAI/bge-m3`** (Hugging Face), **dense Vektorlaenge 1024**, Laden z.B. via `sentence-transformers`; **Alternative** nach Bedarf siehe Abschnitt *Lokales Python-Embedding*. Konfiguration: **`LANGGRAPH_EMBEDDING_*`** wie unter *LM-Studio- und Netbird-Konfiguration*. |
| LangGraph Checkpointing | **Festgelegt:** **SQLite**, nur **lokal** auf diesem Entwicklungsrechner (kein Postgres noetig). |
| Cursor / zusätzliche PR-Reviewer (historisch z.B. **Bugbot**) | **Cursor** und Cursor Cloud Agents sind weiterhin Teil des **aktuellen Mensch-Prozesses** dokumentierbar (**`.cursor/rules/`**, Supervisor-Workflow in der IDE). **`Langgraph_workflow`** hat **keine** Laufzeit-Abhängigkeit von Cursor (**keine** eingebundene Cursor-/Bugbot-API, **kein** solches Feld im Checkpointer-State). Merge-Reife leitet sich aus **Graphen-Schritten** (`run_ci_gates`, LM-/deterministischer Reviewer, optional `gh`-Merge nach Freigabe) ab; GitHub **Actions/CI** und **manuelle** Reviews (auch Bugbot bei Bedarf) bleiben **optional und orthogonal** außerhalb des Pakets. |

---

## Zielbild

LangGraph ersetzt fuer den Team-Workflow die bisherige **Cursor Cloud Agent**-Orchestrierung durch einen **lokalen** Graphen (ohne Cursor als Python-Dependency). Der neue Workflow laeuft lokal bzw. im eigenen Netzwerk:

- **Supervisor Graph:** plant Scopes, kuratiert Kontext, schreibt Handoffs, steuert Gates.
- **Implementation Agent:** arbeitet am `Iron_crab`-Repo, aber nur innerhalb erlaubter Dateien und Regeln.
- **Eval Agent:** arbeitet am `Iron_crab-eval`-Repo, schreibt/erweitert Blackbox- und Invarianten-Tests.
- **Code Reviewer:** prueft PR-/Diff-Zustand, Invarianten, CI-/Testausgaben und fordert Follow-ups an.
- **Memory Layer:** ersetzt nicht Open Brain, sondern nutzt Open Brain weiterhin als Langzeitgedaechtnis.
- **LLM Backend:** LM Studio auf separater AI-Maschine, erreichbar ueber Netbird, idealerweise ueber OpenAI-kompatible API.

Der Rust-Bot wird nicht auf LangGraph portiert. Alle Hot-Path-, Geyser-, Execution-, Strategy- und Eval-Invarianten bleiben unveraendert.

---

## Nicht-Ziele

- Kein Rewrite von `Iron_crab` in Python.
- Keine LangGraph-Komponenten im Trading-Hot-Path.
- Keine direkten Produktions-Deploys durch LangGraph ohne explizite User-Freigabe.
- Keine parallelen Agenten mit demselben Scope, wenn bereits ein aktiver lokaler Agent fuer diesen Scope laeuft.
- Keine Verwischung der Level-5-Trennung: Impl-Agent und Eval-Agent arbeiten mit **getrennten GitHub-Repos, getrennten lokalen Klons, getrennten Indexen** und unterschiedlichen Tool-Berechtigungen (Eval-Agent ohne Zugriff auf Impl-Source-Index und ohne `Iron_crab/src`).

---

## Architektur

### Laufzeit-Komponenten

1. **LangGraph Orchestrator**
   - Python-Prozess oder CLI unter **`C:\Users\Robert Onuk\Langgraph_workflow`**. Dieser Ordner enthaelt **keine zusaetzliche Kopie** von `Iron_crab`/`Iron_crab-eval` als Git-Checkout; **`repo_paths`** zeigen auf die **bestehenden** lokalen Klone (bei dir typisch **`C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab`** und **`...\Iron_crab-eval`** — **Cursor-Workspace** unter `Trading_bot`, falls du diesen Workspace in der IDE so öffnest).
   - Verwaltet `StateGraph`, Checkpoints, Agent-Routing und Human-in-the-loop-Unterbrechungen.
   - Fuehrt lokale Shell-/Git/GitHub-/SSH-/MCP-/Open-Brain-Aktionen ueber klar begrenzte Tools aus.

2. **LM Studio Endpoint**
   - Laeuft auf der AI-Maschine; Zugriff vom Entwicklungsrechner ueber Netbird.
   - **Konfigurierte** OpenAI-kompatible Base URL: `http://100.106.158.144:1234/v1` (Stand Entscheidung; bei IP-/Portwechsel nur Konfig aktualisieren).
   - **Chat-/Completion-Modell** fuer alle Agenten-Rollen vorerst: `qwen3-coder-next` (wie in LM Studio gewaehlt).
   - **Embeddings fuer RAG:** **Default:** `BAAI/bge-m3` (**1024**), Details im Abschnitt *Lokales Python-Embedding*; nicht mit „Chat laeuft“ verwechseln.

3. **Isolierte Arbeitsverzeichnisse (separate GitHub-Repos auf der Platte)**
   - Supervisor schreibt Doku/Plans/Handoffs (typisch Repo **`Iron_crab-eval`**, z.B. `docs/supervisor/`, `docs/plans/`).
   - Impl-Agent arbeitet **ausschliesslich** im lokalen Klon von **`Iron_crab`** (`repo_paths["Iron_crab"]` in Konfiguration).
   - Eval-Agent arbeitet **ausschliesslich** im lokalen Klon von **`Iron_crab-eval`** (`repo_paths["Iron_crab-eval"]`).
   - Optional: **zusaetzlicher Worktree innerhalb desselben Repos** fuer parallele Experimente ist erlaubt, bleibt aber **innerhalb Repo-Grenzen** — die **Isolation ist primaer Repo+Clone+Tool-Policy**, nicht nur Worktree-Ebene.
   - Reviewer arbeitet read-only gegen Diffs, PRs auf GitHub, lokale Logs, Testausgaben und optionally SSH-Log-Snippets.

4. **Persistence / Checkpointing**
   - Pflicht, weil Agenten-, Test-, Review- und Merge-Gates lange laufen koennen.
   - Der Graph-State muss nach Neustart fortsetzbar sein.
   - **Festgelegt:** **SQLite** lokal unter `C:\Users\Robert Onuk\Langgraph_workflow` (nicht innerhalb von `Iron_crab`/`Iron_crab-eval`). Postgres nur wieder relevant, falls der State **spaeter geteilt** werden soll (aktuell nicht geplant).

**SQLite vs. Postgres (kurz):** Bei **nur einem Rechner** ist SQLite einfacher (kein Server, keine Backups anderer Natur als Dateikopie, geringere Betriebsschicht). Postgres lohnt sich bei **mehreren Maschinen** oder **gleichzeitigen Schreibzugriffen** auf denselben Checkpointer — bei eurer Vorgabe entfaellt das vorerst.

### Festgelegter Orchestrator-Pfad

Der LangGraph-/Python-Orchestrator liegt unter:

`C:\Users\Robert Onuk\Langgraph_workflow`

Damit liegen Rust-/Eval-Checkout und Orchestrator-Stamm auf **nachvollziehbar verschiedenen Pfaden** (`Trading_bot\*` vs. `Langgraph_workflow`), auch wenn **Cursor** denselben `Trading_bot`-Ordner als Workspace nutzt.

Konfigurationsdateien (`.env`, Repo-Pfadzeiger, Index-Pfade, SSH-Host-Aliasse) gehoeren weiterhin **`C:\Users\Robert Onuk\Langgraph_workflow`** (nicht in die Repos committen).

### GitHub: Versionskontrolle, Deploy, PR-Politik

- **Versionskontrolle und Deploy** bleiben auf **GitHub** (Branches, Tags, Releases, bestehende Pipelines wie gehabt).
- **Alle Aenderungen, die `Iron_crab` oder `Iron_crab-eval`** im Sinne von **Implementierung, Tests oder Build-veraenderlicher Config** veraendern, laufen ueber einen **Pull Request** in den genehmigten Ziel-Branch.
- **Direktpush (ohne PR)** ist nur zulaessig fuer:
  - **reine Dokumentation** ohne Auswirkung auf Build-/CI-Verhalten (einzelne Repo-Doku unter klaren Naming/Pfad-Regeln), oder
  - Commits auf einem **bewussten Feature-/Topic-Branch**, der noch **nicht** die geschuetzte Hauptlinie erreicht — **Merge in `main`/Release-Branches** weiterhin ueber **PR**.
- Der LangGraph-State soll fuer jeden Merge-relevanten Lauf **`pr_url`**, **Branch**, **Commits** fuehren, damit Supervisor und Review Nachvollziehbarkeit behalten.

### SSH und Server-Logs (Supervisor)

Der Supervisor soll sich per SSH mit dem Produktionsserver verbinden koennen, **Host laut Dokumentation/SSH-Konfig**: **`ironcrab-prod`** (Konfiguration beim User, z.B. `~/.ssh/config`).

**Debugging:** Alle **fuer die Fehlersuche zweckmaessigen Kommandos** sollen nutzbar sein (Logs, Status, fuer Diagnose uebliche Schritte), soweit der SSH-Key/Zugang des Users das auf dem Host erlaubt.

**Ausdrueckliches Verbot im Orchestrator:**

- Kein automatisiertes **Deploy**, kein `deploy.sh` oder aequivalente Release-Schritte **ohne ausdrueckliche User-Freigabe im jeweiligen Turn**.
- Der Graph **loest keinen Deploy** aus — **Deploy fuehrt der User manuell** aus, damit **keine unbeaufsichtigten Produktionsaenderungen** entstehen.

**Umsetzung am Tool:**

- **✅** Modul **`ironcrab_workflow/ssh_debug.py`** (Remote-Befehl auf **IRONCRAB_SSH_HOST**, Default **`ironcrab-prod`**; **`IRONCRAB_SSH_EXTRA_ARGS`**; **`IRONCRAB_SSH_BLOCKED_SUBSTRINGS`** erweiterbar; Output-Cap **`IRONCRAB_SSH_MAX_OUTPUT_CHARS`**).
- Supervisor-Prefetch: wenn **`IRONCRAB_SUPERVISOR_SSH_DEBUG_CMD`** oder **`IRONCRAB_SSH_DEBUG_CMD`** gesetzt sind, dieser Pfad (**ssh** … **-- remote_cmd** mit Blockliste); sonst Fallback **lokaler** Log-Auszug via **`IRONCRAB_SUPERVISOR_SSH_LOG_CMD`** / Timeout **`IRONCRAB_SUPERVISOR_SSH_TIMEOUT`**.

**Uebriges:**

- SSH-Keys nur aus lokaler Umgebung, nicht im Repo oder Graph-State.
- Log-Auszuege koennen trunciert oder redacted werden.
- Timeout und Maximalgroesse fuer Command-Output empfohlen.

---

## Code-Indexierung und Retrieval ✅ (Artefakt-Build + getrennte Suche umgesetzt; Graph-Bindung optional)

Im Gegensatz zu automatischem **Cursor**-Indexing (IDE zur Laufzeit) gibt es hier **kein** Repo-weites Hintergrund-Indexing — **du definierst** explizit, wie Chunks eingespeist werden, wie oft sie neu gebaut werden (z.B. nach `git pull` oder nach Merge von PR), und **welcher Agent welches Retrieval** nutzt.

### Ziele

1. **File-greifende Kontextuelle Arbeit** ohne jedes Mal gesamtes Repo in den Kontext zu legen (Kosten + irrelevante Daten).
2. **Level-5-konforme Isolation**: Eval-Agent darf beim semantischen Suchen **keinen** Embedding- oder Vector-Store nutzen, der den **Rust-Implementierungs-Quellbaum** unter `Iron_crab/src/` enthaelt. Das entspricht derselben Grenze wie ein direkter Dateizugriff: der Store gilt hier als indirekter Zugriff.
3. Der **Supervisor** darf Retrieval aus beiden Welten kombinieren (zwei Stores oder zweistufige Abfragen). Der **Eval-Agent** nutzt nur den Eval-Index und Dateizugriffe im Eval-Repo, um **Blackbox-Level-5** nicht durch Embedding-/Index-Kontamination mit Impl-Source zu brechen.

### Empfohlene Index-Topologie

| Index / Speicher-ID | Quelle (einbinden beim Build) | Lesen durch |
|---------------------|------------------------------|-------------|
| `index_iron_crab_impl` | Vollaendiger nach Policy erlaubter Scope des **Impl-Repos**: `Iron_crab/src/` (nach Policy), Docs wie `Iron_crab/docs/` falls fuer Impl relevant; Ausnahmen konfigurierbar. | Nur **Implementation Agent** (+ optional **Reviewer** bei reinem Impl-Diff wenn Policy erlaubt). **Nicht** Eval-Agent-Tools. |
| `index_iron_crab_eval` | **Eval-Repo**: `Iron_crab-eval/tests/`, `docs/spec/`, `docs/plans/`, Supervisor-Handoffs, `AGENTS.md` — **ohne** Inhalt von impl-internem Source. Optional: eigener kleiner **Public-API**-Snippet-Store aus Docs/Schnittstellenbeschreibung, **ohne** `Iron_crab/src`. | **Eval Agent**, **Supervisor**. |
| `index_supervisor_union` *(optional)* | **Kompakter Planungs-Kontext**: z.B. Invariant-Volltexte, Spec-Auszuege aus Eval-Doku + **oeffentliche** / hochstufige Impl-Beschreibung ohne verbotene Tiefe — oder zwei parallele Queries (Impl + Eval) im Supervisor ohne einen gemischten Embedding-Space. | **Supervisor** (Impl/Eval Agents **nicht** mit Schreib-/Lesewerkzeug fuer diesen Union-Store binden). Alternative: Supervisor nutzt sequenzielle `retrieve(impl)` dann `retrieve(eval)` ohne gemeinsamen physischen Index. |

**Variante ohne dritten kombinierten physischen Index:** Supervisor laedt keine gemischten Vektoren, sondern orchestriert **zwei Retrieval-Aufrufe** gegen `index_iron_crab_impl` und `index_iron_crab_eval` jeweils **read-only**. Das entspricht der Vorstellung (Supervisor darf beides lesen), ohne Datenvermischung in einem Embedding-Space.

### Build und Wartung

- **Nach jedem Pull/Merge oder auf Zeitplan**: Re-Chunk + Re-Embed auf dem **Orchestrator-PC** fuer den betroffenen Repo-Klon; Artefakte unter `Langgraph_workflow\data\` (nicht in den Git-Clones committen).
- **Chunking**: modulbezogen fuer Rust (`mod.rs`-Grenzen) oder Pfad-/Dateiweise mit ueberlapptem Fenster fuer grosse Files.
- **Metadaten** pro Chunk: `repo`, `relative_path`, `commit_sha`, optional `branch` — erleicht Debug und Warnung bei **stalem Retrieval**, wenn sich `commit_sha` und Arbeitsbaum unterscheiden.

### Lokales Python-Embedding auf dem Orchestrator-PC — Systemanforderungen

**Rolle:** Die **Embedding-Pipeline laeuft auf dem gleichen Rechner** wie **`C:\Users\Robert Onuk\Langgraph_workflow`**; **Chat bleibt** auf der LM-Studio-Maschine. Das ist Option **B/D**: technisch **normales Python** plus ein **Embedding-Checkpoint** fuer Vektoren (typisch kleiner als Chat-LLM; oft ONNX-/Transformer auf CPU gut nutzbar, kein zusaetzlicher Chat-Server noetig).

**Software (minimal sinnvoll):**

- **Python** 3.10+ empfohlen **3.11 oder 3.12** (gute Unterstützung aktueller Wheels).
- **Virtuelle Umgebung** unter `Langgraph_workflow\.venv`, reproduzierbar via `pyproject.toml` / `requirements.txt`.
- Embedding-Bibliothek (**Default-Umsetzung**): **sentence-transformers** mit Checkpoint **`BAAI/bge-m3`**, dense **1024-dimensional** fuer den Vector-Store; optional **CUDA** auf dem Orchestrator (z.B. RTX 2080 Ti) fuer Batch-Encoding. Alternativen: FastEmbed/ONNX (CPU-fokussiert) — weniger empfohlen wenn GPU ohnehin vorhanden.

**Hardware — grobe Richtwerte:**

| Ressource | Untergrenze (Indexing geht oft) | Angenehm fuer wiederholtes Re-Embeddings + IDE + Agents |
|-----------|--------------------------------|----------------------------------------------------------|
| **RAM** | 8 GB Gesamt-Rechner (kleines Modell, nicht alles parallel) | **16 GB+** wenn gleichzeitig IDE, Rust-Build und groesseres Embedding-Checkpoint |
| **CPU** | 4 echte Kerne, **SSD** fuer IO | 8 Kerne+: schnelleres Batch-Embeddings |
| **GPU** | **Nicht Pflicht**; viele Python-Embedder sind **CPU/ONNX-first** stark | NVIDIA-GPU kann PyTorch ONNX stark beschleunigen; unter Windows zusätzlicher CUDA/Treiberthema — optional |
| **Platte** | +**2–5 GB** fuer Modell-Cache (HuggingFace o.ae.) plus **einige GB** fuer Vektor-/Index-Artefakte | **SSD** fuer `data\index_*`; bei sehr grossem Corpus mehr Reserve |
| **Netz** | Einmal Download der Embedding-Gewichte; danach Offline moeglich | — |

**Betrieb:**

- Embedding laeuft **batchweise** beim `index_builder` (kein durchgehender Daemon noetig). RAM-Spitzen = Modell laden + Chunk-Batch gleichzeitig.
- Indices und Checkpoints gemeinsam mit LangGraph nur lokal pflegen und **Backup/AV-Ausnahmen** fuer `Langgraph_workflow\data\` ueberlegen (Windows Defender kann grosse rekursive Schreibzugriffe verlangsamen beim ersten Aufbau).

**Default-Vorschlag (festgehalten):**

| Feld | Wert |
|------|------|
| Hugging-Face-/Modell-ID | **`BAAI/bge-m3`** |
| Dense Embedding-Dimension | **1024** (fuer Persistenz/Vektor-DB konstant halten) |
| Empfohlene Bibliothek | **sentence-transformers** + PyTorch (**CUDA**, falls auf dem Orchestrator verfuegbar) |

**Optionaler Wechsel** nur nach Retrieval-Vergleich: z.B. **`nomic-ai/nomic-embed-text-v1.5`** (langer Kontext) oder **`Snowflake/snowflake-arctic-embed-l-v2.0`** — dann **`LANGGRAPH_EMBEDDING_DIMENSION`** und Store neu aufbauen.

Konfigurationsvariablen (Beispiele): **`LANGGRAPH_EMBEDDING_BACKEND=sentence_transformers`**, **`LANGGRAPH_EMBEDDING_MODEL=BAAI/bge-m3`**, **`LANGGRAPH_EMBEDDING_DIMENSION=1024`**, optional **`LANGGRAPH_EMBEDDING_DEVICE=cuda`** bzw. `cpu`, sowie **`LANGGRAPH_HF_HOME`** fuer Cache-Pfad unter `Langgraph_workflow`.

### Tool-Schnittstelle fuer Agents ✅ / ⏳

- ✅ Python-API **`search_iron_crab_impl`** / **`search_iron_crab_eval`** (Modul **`ironcrab_workflow.indexing.search`**) — Mapping wie:
  - `semantic_search(repo="Iron_crab", …)` → nur **`index_iron_crab_impl`**
  - `semantic_search(repo="Iron_crab-eval", …)` → nur **`index_iron_crab_eval`**
- ✅ optionaler Supervisor-Vorspann: **`IRONCRAB_SUPERVISOR_LM_INDEX_PREFETCH=1`** (Top‑K **`IRONCRAB_SUPERVISOR_LM_INDEX_TOP_K`**, Standard 6) beim LM-Supervisor-Scope-Kontext.
- ⏳ Explizites LangChain-„Tool“-Objekt fuer spaetere Agents — aktuell reine Funktionsbindung aus Python.

Eval-Agent-Code soll **kein** direktes `search_iron_crab` importieren/nutzen (**Level 5**: kein Zugriff auf Impl-Source-Chunk-Store über Embeddings).

Zusaetzliche Tools `read_file`, `glob`, `run_tests` bleiben am jeweiligen `repo_paths[...]` haengen — dieselbe Zugriffslogik wie der Index.

---

## State-Modell

Der zentrale LangGraph-State sollte mindestens enthalten:

```python
class WorkflowState(TypedDict, total=False):
    task_id: str
    user_request: str
    current_scope: str
    target_repo: Literal["Iron_crab", "Iron_crab-eval", "supervisor"]
    phase: str
    pr_url: str
    repo_paths: dict[str, str]
    branch_names: dict[str, str]
    handoff_path: str
    handoff_text: str
    relevant_invariants: list[str]
    known_bug_patterns: list[str]
    memory_hits: list[dict[str, Any]]
    agent_runs: list[dict[str, Any]]
    changed_files: list[str]
    test_commands: list[str]
    test_results: list[dict[str, Any]]
    ci_status: dict[str, Any]
    review_findings: list[dict[str, Any]]
    blockers: list[str]
    requires_user_decision: bool
    next_action: str
    needs_public_api_skeleton: bool  # true wenn neue/geaenderte Public API fuer kompilierbare Eval-Tests noetig ist
```

Wichtig: Der State speichert keine Secrets (keine SSH-Keys, keine GitHub-Tokens als Klartext im Checkpoint). API Keys, LM-Studio-URL, Netbird, SSH-Host-Alias und Repo-Pfade kommen aus Environment-Variablen oder Konfiguration unter `C:\Users\Robert Onuk\Langgraph_workflow`.

---

## Graph-Struktur

### Hauptgraph

Der erste produktive Graph sollte diese Nodes haben:

1. `load_workspace_rules`
   - Liest Supervisor-Regeln, AGENTS-Dateien und projektspezifische STOP-CHECK-Regeln.
   - Ergebnis: explizite Regel-Liste im State.

2. `classify_request`
   - Entscheidet, ob der User eine Planung, Impl-Aenderung, Eval-Aenderung, Review, Merge oder Deploy-Freigabe meint.
   - Setzt `target_repo`, `current_scope`, `requires_user_decision`.

3. `gather_context`
   - Liest relevante Specs, Plans, `Tests_todo.md`, `KNOWN_BUG_PATTERNS.md`.
   - Fragt Open Brain per `semantic_search` nach Failure-Patterns und Architekturentscheidungen.

4. `write_handoff`
   - Erstellt ein Handoff mit Pflichtabschnitten:
     - Regel-Verweis als erstes.
     - Task-Beschreibung.
     - Relevante Invarianten im Volltext.
     - Bestehende Patterns.
     - Erlaubte Dateien.
     - Verbotene Aktionen.
     - Pruef-Befehle.

5. `route_to_worker`
   - Routed anhand des Scopes zu Impl-Agent, Eval-Agent oder Reviewer.
   - Verhindert Doppelstarts fuer denselben Scope.

6. `run_impl_agent`
   - Arbeitet im lokalen Klon des GitHub-Repos `Iron_crab` (Festplattenpfad in `repo_paths["Iron_crab"]`).
   - Darf Rust-Implementierung aendern.
   - Muss lokale Checks ausfuehren.

7. `run_eval_agent`
   - Arbeitet im lokalen Klon von `Iron_crab-eval` (`repo_paths["Iron_crab-eval"]`).
   - Darf Tests/Specs gemaess Eval-Regeln aendern.
   - Darf keine Impl-Details aus `Iron_crab/src` lesen.

8. `run_code_reviewer`
   - Prueft geaenderte Dateien, Invarianten und Testresultate.
   - Liefert Findings als strukturierte Daten.

9. `run_required_checks`
   - Fuer `Iron_crab`:
     - `cargo fmt --check`
     - `cargo clippy -- -D warnings`
     - `cargo test`
     - volle Eval-Suite gegen passenden Eval-Checkout
   - Fuer `Iron_crab-eval`:
     - `cargo fmt -p ironcrab-eval -- --check`
     - `cargo check`
     - `cargo build`
     - `cargo clippy -p ironcrab-eval`

10. `decide_followup_or_merge_ready`
    - Wenn Findings oder rote Checks existieren: Follow-up an denselben lokalen Agenten.
    - Wenn alles gruen ist: Review-Gate abgeschlossen.
    - Deploy bleibt immer Human-in-the-loop.

11. `persist_memory`
    - Speichert neue Failure-Patterns, Architekturentscheidungen und Invarianten-Evolution in Open Brain.

12. `interrupt_for_user`
    - Stoppt kontrolliert bei echten Entscheidungen:
      - Deploy-Freigabe.
      - Unklarer Scope.
      - Manuelle Nachbearbeitung **ausserhalb** LangGraph (z.B. **`gh`-Kommentare**, Team-Review, optional bei dir **Bugbot** auf GitHub) — **nie** als verpflichtender Graph-Schritt oder API-Kopplung im Orchestrator-Paket.
      - Konflikt zwischen Regeln.

### Routing-Regeln

#### Standard-Workflow pro Scope (Hybrid, Level-5-konform)

Konzeption **Spec und Vertrag zuerst**, **Compile** auf der Eval-Seite **nur so viel Impl** wie fuer typisierte Tests noetig, **Fertigstellung** erst wenn **Eval gruen** (plus Impl-lokale Gates).

1. **Spec / Invariante klaeren** — Supervisor inkl. Draft-Handoff, relevante Invarianten im Volltext, erlaubte Dateien.
2. **Eval konzeptionell zuerst** — oeffentliche API, geplante Tests, Szenarien und erwartetes Verhalten **ohne** `Iron_crab/src` zu lesen.
3. **Minimaler Impl-Schritt (nur wenn noetig)** — PR oder Branch auf `Iron_crab` mit **nur ausreichend oeffentlicher Oberflaeche** (Signaturen; klarer `panic!` / `todo!` / Fehlerpfad), sodass `ironcrab-eval` **`cargo check`** und **`cargo test`** **starten** kann (kein vollstaendiges Verhalten erwartet). Entfaellt, wenn die Aenderung **ausschliesslich** die **bestehende** Public API nutzt.
4. **Eval-PR / Eval-Aenderung** — Tests so schreiben, dass sie **korrekt gegen die Spec fehlschlagen** (Assertions / erwartetes Rot), solange die Implementation noch fehlt oder falsch ist.
5. **Impl bis gruen** — Impl-Agent implementiert, bis **Eval-Suite gruen** ist und die **Impl-lokalen Gates** (fmt, clippy, unit tests, wo anwendbar volle Eval-Level-5-Kette) bestehen.

**Graph-Routing:** Conditional Edges zwischen `write_handoff`, `run_impl_agent` (Skeleton), `run_eval_agent`, `run_impl_agent` (Vollimplementierung) und `run_code_reviewer` folgen den Phasen **1–5**; Phase 3 wird uebersprungen, wenn `needs_public_api_skeleton == false` im State.

#### Weitere Routing-Regeln

- **Code Reviewer** nach jedem abgeschlossenen Agenten-Lauf (Skeleton-PR, Eval-PR, Impl-Follow-up), bevor der Scope als merge-ready gilt.
- Kein neuer Agent fuer denselben Scope, solange der vorherige Lauf nicht abgeschlossen oder explizit abgebrochen ist.
- Bei Regelverstoss kein Auto-Fix durch Supervisor, sondern Follow-up an den zustaendigen Agenten.
- **Reihenfolge der PRs auf GitHub:** typisch **Minimal-Impl-PR** (optional) **vor** dem **Eval-PR**, der neue Symbole braucht; danach **ein oder mehrere Impl-PRs** bis Eval gruen. Bei nur bestehender API: **Eval-PR** kann vor dem massgeblichen Impl-PR stehen, wenn Tests sofort kompilieren.

---

## Agenten-Rollen

### Supervisor Agent

Verantwortung:

- Zerlegt User-Ziele in kleine Scopes.
- Kuriert Kontext aus Specs, Plans, Memory und Bug-Patterns.
- Schreibt Handoffs.
- Steuert Graph-Routing, Checkpoints und Gates.
- Schreibt keine Rust-Impl und keine Eval-Tests selbst, sofern die bestehende Rollentrennung beibehalten wird.

Erlaubt:

- `Iron_crab-eval/docs/` aktualisieren.
- Handoffs und Plaene schreiben.
- Open Brain lesen/schreiben.
- Workflows starten und pruefen.
- Ueber ein konfiguriertes Tool **SSH fuer Debugging nach `ironcrab-prod`**, keine Deploy-Automatisierung ohne ausdrueckliche User-Freigabe im Turn.

### Implementation Agent

Verantwortung:

- Implementiert konkrete Rust-Aenderungen in `Iron_crab`.
- Folgt `AGENTS.md`, `.cursor/rules/ironcrab-core.mdc` und Handoff.
- Fuehrt lokale Rust-Checks aus.
- Meldet Regelkonflikte statt sie zu umgehen.

Isolation:

- Eigener physischer **Git Clone** nur von `Iron_crab` nach `repo_paths["Iron_crab"]`; keine Aenderungen im Eval-Klon durch diesen Agenten.
- Zugriff auf semantisches Retrieval nur ueber **`index_iron_crab_impl`** (keine Eval-/Blackbox-Spoiler aus dem anderen Index noetig).
- Kein Zugriff auf Eval-Tests als Designvorlage, wenn dadurch Blackbox-Grenzen verwischt wuerden.

### Eval Agent

Verantwortung:

- Schreibt/erweitert Spec- und Eval-Tests in `Iron_crab-eval`.
- Testet Invarianten an der oeffentlichen API.
- Liest keine Implementierungsdetails aus `Iron_crab/src`.

Isolation:

- Eigener physischer **Git Clone** nur von `Iron_crab-eval`; kein Zugriffs-Tool fuer `semantic_search`/Index von `Iron_crab`.
- Nur erlaubte Doku aus `Iron_crab/docs/` (oberhalb von `src`) und Nutzung der **oeffentlichen** `ironcrab`-API in Tests wie in `AGENTS.md` beschrieben.

**Abgrenzung:** Der Eval Agent **schreibt** Spec-/Eval-Tests; er **bewertet** nicht die Repo-weite Merge-Fitness als separater Rollenakteur (dafuer **Code Reviewer** nach Graph-Routing).

### Code Reviewer (Review Agent)

**Eigener Graph-/Agenten-Schritt:** `run_code_reviewer` — nicht mit dem Eval Agent verwechseln.

Verantwortung:

- Prueft **Diffs**, PR-Zusammenhang, CI-/Testausgaben und Logs gegen **Handoff**, Projektregeln und **Invarianten**.
- Priorisiert Bugs, Regressionen, fehlende Tests und Regelverstoesse; liefert strukturierte **Findings**.
- Entscheidet nicht automatisch gegen klare STOP-CHECK-Regeln; bei Verstoss **blocked** oder **changes_requested**, kein Umgehen der Regeln.

**Abgrenzung zum Eval Agent:** Der Code Reviewer **uebernimmt nicht** das Verfassen von Spec-/Eval-Testcode statt des Eval Agenten; er **reviewt** die vom Eval- und Impl-Agenten erzeugten Aenderungen. Fehlende oder unzureichende Tests werden als **Finding** beschrieben; die Umsetzung erfolgt durch den **Eval Agent** (oder eine neu beauftragte Runde ueber den Supervisor).

Review-Ausgabe (Beispielschema):

```json
{
  "status": "approved | changes_requested | blocked",
  "findings": [
    {
      "severity": "critical | high | medium | low",
      "file": "path",
      "reason": "konkretes Problem",
      "required_fix": "konkrete Erwartung"
    }
  ],
  "checks_required": ["cargo fmt --check", "cargo test"],
  "user_decision_required": false
}
```

---

## LM-Studio- und Netbird-Konfiguration

### Environment

Empfohlene lokale Variablen:

```powershell
$env:LANGGRAPH_LLM_BASE_URL = "http://100.106.158.144:1234/v1"
$env:LANGGRAPH_LLM_MODEL = "qwen3-coder-next"
$env:LANGGRAPH_LLM_API_KEY = "lm-studio"
$env:LANGGRAPH_CHECKPOINT_DB = "sqlite:///./.langgraph/checkpoints.sqlite"

# Lokale Repo-Roots — gleicher Baum wie typischer Cursor-Workspace unter Desktop (Default)
$env:LANGGRAPH_REPO_IRON_CRAB = "C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab"
$env:LANGGRAPH_REPO_IRON_CRAB_EVAL = "C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab-eval"

# Embedding (Orchestrator-PC, lokaler Python-Indexer — Default-Vorschlag)
$env:LANGGRAPH_EMBEDDING_BACKEND = "sentence_transformers"
$env:LANGGRAPH_EMBEDDING_MODEL = "BAAI/bge-m3"
$env:LANGGRAPH_EMBEDDING_DIMENSION = "1024"
$env:LANGGRAPH_EMBEDDING_DEVICE = "cuda"
# Fallback ohne GPU: $env:LANGGRAPH_EMBEDDING_DEVICE = "cpu"
# Optional: HF-Cache unter dem Workflow-Verzeichnis
# $env:HF_HOME = "C:\Users\Robert Onuk\Langgraph_workflow\.cache\huggingface"
```

### Connectivity-Check

Vor jedem Agentenlauf:

```powershell
Invoke-RestMethod `
  -Uri "$env:LANGGRAPH_LLM_BASE_URL/models" `
  -Headers @{ Authorization = "Bearer $env:LANGGRAPH_LLM_API_KEY" }
```

Erwartung:

- Die AI-Maschine ist ueber Netbird erreichbar.
- LM Studio liefert das geladene Modell.
- Fehler beim Connectivity-Check stoppen den Graph vor kostenintensiver Arbeit.

---

## Ordnerlayout auf der Entwicklungsmaschine

### Orchestrator-Stammverzeichnis (fest)

```text
C:\Users\Robert Onuk\Langgraph_workflow\
```

Alle Python-/LangGraph-Artefakte liegen **`C:\Users\Robert Onuk\Langgraph_workflow`** (venv, Checkpoint-DB, Index-Caches, `.env`). Die **Repos** werden **von dort aus nur referenziert** (`repo_paths`/`LANGGRAPH_REPO_*`), typisch auf **`C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab`** und **`...\Iron_crab-eval`** — dieselben Pfade wie dein lokaler Arbeitsbaum unter `Trading_bot`.

### Lokale Repo-Checkout-Pfade (**Default fuer dieses Setup**)

Konfigurationswerte (Beispiele fuer `.env`; anpassbar bei Umzug):

```text
LANGGRAPH_REPO_IRON_CRAB     = C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab
LANGGRAPH_REPO_IRON_CRAB_EVAL = C:\Users\Robert Onuk\Desktop\Trading_bot\Iron_crab-eval
```

Entspricht **`repo_paths["Iron_crab"]`** und **`repo_paths["Iron_crab-eval"]`** im Graphen-State. Beide liegen unter dem Desktop-Ordner **`Trading_bot`**, den **Cursor** lokal bereits öffnen kann — LangGraph **klont** die Repos dort **nicht erneut**, sondern verwendet diese Pfade.

### Empfohlene Struktur innerhalb von `Langgraph_workflow`

```text
C:\Users\Robert Onuk\Langgraph_workflow\
  pyproject.toml
  README.md
  .env.example
  data\
    checkpoints\          ← SQLite oder aehnliche Checkpointer-Persistenz
    index_iron_crab_impl\    ← Build-Artefakte des Impl-Indexes (einbinden nach Policy)
    index_iron_crab_eval\    ← Build-Artefakte des Eval-Indexes
    # optional index_supervisor_union\
  src\
    workflow\
      __init__.py
      config.py
      state.py
      graph.py
      nodes/
        load_rules.py
        classify_request.py
        gather_context.py
        write_handoff.py
        run_impl_agent.py
        run_eval_agent.py
        run_reviewer.py
        run_checks.py
        persist_memory.py
      tools/
        shell.py
        git.py
        github.py
        ssh_debug.py
        openbrain.py
        lmstudio.py
        retrieve_impl.py       ← nur gegen index_iron_crab_impl / Impl-Filesystem
        retrieve_eval.py       ← nur gegen index_iron_crab_eval / Eval-Filesystem
        index_builder.py       ← CLI oder Node zum Neu-Generieren einzelner Indexes
      prompts/
        supervisor.md
        implementation_agent.md
        eval_agent.md
        code_reviewer.md
  tests/
    test_graph_routing.py
    test_scope_deduplication.py
    test_handoff_required_sections.py
    test_human_interrupts.py
    test_eval_agent_no_impl_retrieval.py   ← Policy: kein Zugriff auf Impl-Retrieval-Tools
```

**Hinweis (umgesetzt Mai 2026):** Produktiver Code liegt unter **`src/ironcrab_workflow/`** (Flachbau statt Unterordners `workflow/`); Index-Build liegt in **`ironcrab_workflow/indexing/`**, CLI **`python -m ironcrab_workflow.indexing build …`**.

---

## Rollout-Plan

### Phase 0: Architektur festschreiben

**Status:** Abgehakt (Checkliste siehe DoD-Kaestchen unten). **Hinweise:** SSH-Zugang `ironcrab-prod` und GitHub-Berechtigungen sind **Nutzer-/Maschinen-spezifisch** — dort bei Bedarf lokal gegenprüfen (`ssh ironcrab-prod`, `git remote -v` in beiden Repos). Operative Index-Pfade beim Build eines `index_builder` siehe weiterhin Abschnitt *Code-Indexierung und Retrieval*; optional `LANGGRAPH_INDEX_*` in `Langgraph_workflow/.env.example`.

Ziel:

- Dieses Dokument fuer den aktuellen Stand reviewen (**Phase-0-Entscheidungen** siehe Kopf dieser Datei).
- Ordner **`C:\Users\Robert Onuk\Langgraph_workflow`** anlegen (leer oder mit minimalem README), ohne Dateien aus `Iron_crab`/`Iron_crab-eval` hineinzukopieren.
- Zwei **getrennte lokale Klone** der GitHub-Repos verifizieren (Impl und Eval): `git remote -v`, Default-Branches, Zugriffsrechte.
- **GitHub-Arbeitsweise** dokumentieren wie oben beschrieben (Impl/Eval **immer PR**, Direktpush nur Doku ohne Build-/CI-Auswirkung oder Feature-Branch + spaeter PR).
- **SSH-Debugging** einrichten: Test `ssh ironcrab-prod`; keine Deploy-Schritte im Orchestrator-Werkzeug; Hostname in lokaler SSH-Config dokumentiert.
- **Indexing-Plan** konkret machen (welche Pfade werden in welchen Store eingeschlossen/ausgeschlossen; Rebuild nach Pull/Merge; Tool-Bindung pro Rolle gemaess Abschnitt *Code-Indexierung und Retrieval*).

DoD (Phase 0):

- [x] Orchestrator-Pfad **`C:\Users\Robert Onuk\Langgraph_workflow`** existiert als vereinbarter Stamm und ist im Team/Docs referenziert.
- [x] Repo-Pfade fuer beide Klone sowie optionale Index-Pfad-Variablen sind in **`.env.example`** dokumentiert (`LANGGRAPH_REPO_IRON_CRAB`, `LANGGRAPH_REPO_IRON_CRAB_EVAL`; optional kommentierte `LANGGRAPH_EMBEDDING_*` und Index-Artefakt-Hinweise unter `Langgraph_workflow\data\index_*`).
- [x] LM-Studio Chat: Base URL **`http://100.106.158.144:1234/v1`**, Modell **`qwen3-coder-next`** fuer alle Agenten dokumentiert — nicht mit Secrets gemischt (ggf. hoechstens IP Rotation in eigener lokaler Kopie von `.env.example`).
- [x] **Embeddings fuer Indizes:** Default **`BAAI/bge-m3`** (**1024**), Variablen `LANGGRAPH_EMBEDDING_*` wie im Environment-Block; ✅ **`ironcrab_workflow.indexing`** Builder + Artefakte (`pip install -e ".[index]"`, siehe **`Langgraph_workflow/README.md`**).
- [x] Sicherheitsgrenzen fuer Agentenrollen, **Repo-Isolation**, **Indexing-Isolation** Eval vs. Impl sowie **SSH auf `ironcrab-prod` ohne automatisches Deploy** sind in diesem Plan, in **`.cursor/rules/`** (Supervisor-Doku) und in Repo-Invarianten/Doku fuer den Rollout beschrieben; **`Langgraph_workflow`** bleibt davon zur Laufzeit unabhängig.

### Phase 1: Minimaler LangGraph-Skeleton

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`** (Python-Paket `ironcrab_workflow`, Tests in `tests/`).

Ziel:

- Graph mit State, Checkpointer und drei Nodes:
  - `classify_request`
  - `gather_context`
  - `interrupt_for_user`

Tests:

- Request "fix impl bug" routed zu `Iron_crab`.
- Request "add invariant test" routed zu `Iron_crab-eval`.
- Request "deploy" setzt `requires_user_decision = true`.

DoD:

- `python -m pytest` im Ordner **`Langgraph_workflow`**: Routing + Checkpoint **gruen**; LM-`/models`-Connectivity **laeuft mit** wenn Netbird **`LANGGRAPH_LLM_BASE_URL`** erreicht (siehe `.env`/`conftest`; Ausnahme: `IRONCRAB_SKIP_LM_STUDIO_CHECK=1`).
- Checkpointer: SQLite schreibt `*.sqlite`; State laesst sich nach erneutem Compile mit derselben DB ueber **`get_state`** lesen (Nachweis „Resume“ fuer Phase 1).

### Phase 2: Handoff-Generator

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`** (`ironcrab_workflow/handoff.py`, Knoten `generate_handoff`; Tests `tests/test_handoff.py`).

Ziel:

- Automatisch Handoffs nach bestehendem Pflichtschema erzeugen.
- Handoff wird in `Iron_crab-eval/docs/supervisor/` geschrieben.

Ausloesung im Graph:

- Nur wenn `handoff_generate: true` gesetzt ist (sonst keine Datei; Routing/Phase-1-Flow bleibt unveraendert).
- Pflichtfelder fehlen → **Blocker** (`requires_user_decision: true`, keine leere/unvollstaendige Markdown-Datei).

Tests:

- Handoff beginnt mit Regel-Verweis.
- Invarianten werden im Volltext eingefuegt.
- Erlaubte/verbotene Dateien sind enthalten.
- Pruef-Befehle sind enthalten.

DoD:

- Kein Handoff ohne Pflichtabschnitte (Validierung vor Schreibzugriff).
- Fehlende Invarianten fuehren zu Blocker statt leerem Handoff (`handoff_generate: true`).

### Phase 3: Lokaler Impl-Agent

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`**: Knoten **`run_impl_agent`** nach `generate_handoff`; Module `handoff_parse.py`, `impl_executor.py`; Tests `tests/test_impl_agent.py`.

Ziel:

- Implementation Agent arbeitet im Konfigurationspfad `repo_paths["Iron_crab"]` (lokalen **Git Clone** desselben Repo wie GitHub Impl).
- Agent bekommt Handoff und darf nur erlaubte Dateien aendern.
- Shell-Kommandos werden geloggt.

Aktivierung (Graph-State):

- `impl_agent_requested: true`; Handoff aus `impl_agent_handoff_path` **oder** (nach erfolgreicher Phase 2 im selben Laufs) `handoff_written_path`.
- `impl_agent_plan`: relativer Pfad unter dem Impl-Klon zu Textinhalt (`dict[str, str]`).
- Optional `impl_agent_shell_commands`: Liste von Kommandoargumentvektoren; Exit-Code ungleich Null blockiert (`requires_user_decision`). Trockenlauf ohne echten Subprocess: **`IRONCRAB_IMPL_AGENT_SHELL_DRY_RUN=1`** (tests/CI).

Tests:

- Agent wird blockiert, wenn er eine verbotene Datei aendern will.
- Agent wird blockiert, wenn er ohne Handoff startet.
- Agent kann einen Dummy-Doku-Scope in einem Test-Workspace bearbeiten.

DoD:

- Impl-Scopes laufen **ohne Cursor Cloud Agent API** ueber lokale Knotenwerkzeuge (`run_impl_agent`); PRs/GitHub wo weiterhin gefordert.
- Checkpointing: gleicher SQLite-/Memory-Adapter wie uebriger Graph (`impl_shell_log` und Handoff-State im Thread-Snapshot nachweisbar).
- Parallelstart eines zweiten **Cursor Cloud Agents** fuer denselben Scope entfaellt fuer diesen lokalen Executor (kein automatischer zweiter Spawn).

### Phase 4: Lokaler Eval-Agent

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`**: Knoten **`run_eval_agent`** nach `run_impl_agent`; Module `handoff_parse.py` (`extract_eval_target_paths`), `eval_executor.py`; Tests `tests/test_eval_agent.py`.

Ziel:

- Eval Agent arbeitet im Konfigurationspfad `repo_paths["Iron_crab-eval"]` (lokalen **Git Clone** des Eval-Repos auf GitHub).
- Eval-Agent-Regeln werden vor jeder Aenderung geprueft (Handoff-Allowlist aus `## Zieldatei(en)`).
- Kein Zugriff auf den Impl-Source-Checkout (`Iron_crab/src` als Pfadsegment verboten; Resolve darf Impl-Root nicht treffen wenn konfiguriert).

Aktivierung (Graph-State):

- `eval_agent_requested: true`; Handoff `eval_agent_handoff_path` oder `handoff_written_path` (nach Phase 2, Eval-Ziel).
- `eval_agent_plan: dict[path, contents]` relativ zum Eval-Klon.
- Optional `eval_agent_shell_commands`; `eval_agent_run_slim_gates` (Standard True) fuer die vier Gates aus `eval_executor.SLIM_EVAL_GATES_DEFAULT` (entspricht dem schlanken Eval-Gate auf GitHub: ohne `--all-targets`).
- Standard: Pfadliste muss Unterordner **`tests/`** sein; fuer Doku-Sonderfaelle **`IRONCRAB_EVAL_AGENT_ALLOW_NON_TESTS=1`**.

Tests:

- Zugriff auf `Iron_crab/src` wird als Regelverstoss geblockt.
- Aenderungen an Testdateien erfolgen nur ueber Eval-Handoff-Allowlist unter `tests/` (Impl-Agent arbeitet im anderen Repo-Klon).
- Schlanke Eval-Gates werden lokal ausgefuehrt (Auswertung der Exit-Codes wie Phase 3 Shell).

DoD:

- Eval-Scopes **ohne geschlossene Cloud-Agent-API** ueber lokale Knotenwerkzeuge abschliessbar (`run_eval_agent`).
- `eval_shell_log` und Status im gleichen Checkpoint-Thread wie uebrige Knoten nachweisbar.
- Merge-/Reviewer-Flow fuer Testdiff-Einsicht kann auf Phase 5 folgen (`eval_agent_touched_files` + Logs im State).

### Phase 5: Code Reviewer Graph

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`**: Knoten **`code_reviewer`** nach `run_eval_agent`, `reviewer_router_pick_next` (conditional_edges Impl/Eval/Interrupt), Policy in `reviewer.py`, State-Felder in `WorkflowState`, Tests `tests/test_reviewer.py`.

Ziel:

- Reviewer prueft jeden Agentenlauf gegen Handoff, Diffs, Invarianten und Checks.
- Findings werden maschinenlesbar in den State geschrieben.

Tests:

- Finding `changes_requested` routed zurueck an denselben Agenten.
- `approved` mit roten Checks bleibt blockiert.
- `approved` mit gruenen Checks wird merge-ready.

DoD:

- Kein Scope gilt als erledigt ohne Reviewer-Entscheidung.
- Kritische Invariantenverstoesse erzeugen Follow-up, nicht Merge.

### Phase 6: CI-/Gate-Integration

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`**: Knoten **`run_ci_gates`** nach **`run_eval_agent`**, Modul **`ci_gates.py`**, Status-Felder in **`WorkflowState`**, Befuellung von **`review_checks_passed`** bei Gate-Lauf; Tests **`tests/test_ci_gates.py`**; Standard in pytest **`IRONCRAB_CI_GATES_SKIP=1`**.

Ziel:

- Lokale Gates werden vom Graph ausgefuehrt und ausgewertet.
- Optional GitHub Checks weiter beobachten, falls PRs weiter genutzt werden.

DoD fuer `Iron_crab`:

- `cargo fmt --check` gruen.
- `cargo clippy -- -D warnings` gruen.
- `cargo test` gruen.
- Volle Eval-Suite gegen passenden Eval-Checkout gruen.

DoD fuer `Iron_crab-eval`:

- `cargo fmt -p ironcrab-eval -- --check` gruen.
- `cargo check` gruen.
- `cargo build` gruen.
- `cargo clippy -p ironcrab-eval` gruen.

### Phase 7: Memory-Integration

**Status:** Umgesetzt unter **`C:\Users\Robert Onuk\Langgraph_workflow`**: Knoten **`prefetch_openbrain_for_handoff`** vor **`generate_handoff`**, **`persist_openbrain_pipeline`** nach **`run_ci_gates`**; Modul **`openbrain_client.py`** (asyncpg/pgvector, optional Extra **`[openbrain]`**); Handoff-Abschnitt **`## OpenBrain-relevante Treffer`**; State-Felder **`handoff_memory_hits`**, **`openbrain_*`**; Tests **`tests/test_openbrain_phase7.py`**; pytest isoliert OpenBrain per **`conftest.py`**.

Ziel:

- Open Brain bleibt Source fuer Failure-Patterns und Architekturentscheidungen.
- Graph schreibt neue Patterns nach Eval-/CI-Fails.

DoD:

- Bei wiederkehrendem Fehler wird `failure_pattern` gespeichert.
- Bei Architekturentscheidung wird `architectural_decision` gespeichert.
- Handoffs enthalten relevante Memory-Hits.

**Operative naechste Schritte (lokal):** Im Orchestrator-Repo **`Langgraph_workflow/README.md`** — Abschnitt *Naechste Schritte (nach erfolgreichem Smoke)*: **`scripts/smoke_invoke.py`** (Routing + Gates), **`scripts/handoff_invoke.py`** (Handoff-Datei + voller Graph), optional Open Brain / Agent-State laut README.

### Adoption (ersetzt „Phase 8 Parallel / Phase 9 Abbau“)

**Logik:** Sobald ihr den LangGraph‑Workflow fuer **die Orchestrierung** nutzt, **endet** Cursor Cloud Agent fuer diese Arbeit ohnehin — **`Langgraph_workflow`** enthält **keinen** Aufruf, Cursor Cloud Agents zu starten. **„Phase 9 Cursor abbauen“** ist daher keine zweite Implementierungs‑Phase fuer den Graphen, sondern nur noch **„keine Cursor‑Cloud‑Agenten‑Runs mehr fuer dieselben Scopes ausloesen“**.

- **Parallelbetrieb nur temporaer beim Uebergang:** Fuer **denselben** Scope nie LangGraph‑Executor **und** Cursor Cloud Agent mischen — das ist Doppelarbeit und Verwirrung (Regel beim Team, keine extra Architekt‑Phase).

- **Cursor IDE ohne Cloud‑Orchestrierung** oder ein anderer Editor: bleibt **optional** (**`.cursor/rules/`** weiter als Doku); der Orchestrator davon **unabhaengig**.

**Optional zur Selbst-Verifikation (kein formelles DoD):** wenige echte kleine Scopes mit LangGraph, einmal Checkpoint/Resume nach Neustart anfassen — gibt Vertrauen, ist aber **nicht** die alte Phase‑8‑„mindestens drei Scopes“‑Ueberwachung.

**Implementierung im Repo** (bereits umgesetzt, siehe **`Langgraph_workflow`/README`): OpenBrain‑Prefetchs, Supervisor‑LM, **`generate_handoff`**, Hybrid **`IRONCRAB_HYBRID_LEVEL5`**, **`run_impl_agent`**, **`run_code_reviewer_lm`**, **`run_eval_agent`**, **`run_ci_gates`**, **`persist_openbrain_pipeline`**, **`code_reviewer`**, optional **`gh pr merge`**, Code‑Indexierung **`ironcrab_workflow.indexing`**, **`ssh_debug`**, **`gather_context`**.

---

## Sicherheits- und Kontrollregeln

1. **Secrets**
   - Keine API Keys im Graph-State.
   - Keine Secrets in Handoffs.
   - `.env` bleibt lokal und wird nicht committed.

2. **Netbird / LM Studio**
   - LM Studio nur ueber Netbird erreichbar machen, nicht offen ins LAN/Internet.
   - Vor Agentenlauf Connectivity pruefen.
   - Bei Modellwechsel Modellname im Run-State dokumentieren.

3. **Agenten-Isolation**
   - Pro Rolle **eigener physischer Git-Clone eines GitHub-Repos** (Impl vs. Eval) plus **eigene Retrieval-/Index-Zugriffsrechte** (Eval ohne Impl-Retrieval binden siehe Abschnitt *Code-Indexierung und Retrieval*).
   - Der Orchestrator-Code unter **`C:\Users\Robert Onuk\Langgraph_workflow`** enthaelt keine produktiven Bot-Sources aus den Repos ausserhalb von Konfig-/Cache.
   - Pro Scope maximal ein aktiver Worker-Agent.
   - Reviewer read-only auf Diffs/Ausgaben; Supervisor **SSH fuer Debugging nach `ironcrab-prod`**, ohne Deploy-Automatisierung.

4. **Produktionsschutz**
   - Deploy bleibt Human-in-the-loop.
   - Kein automatischer systemd-Restart.
   - Kein `deploy.sh` ohne explizite Freigabe im aktuellen Turn.

5. **Rust-Hot-Path-Schutz**
   - LangGraph darf keine Runtime-Abhaengigkeit des Bots werden.
   - Kein Python-/LLM-Aufruf im Trading-Flow.
   - Invarianten I-4, I-7, I-9 und I-12 bleiben Review-Pflicht.

---

## Offene Entscheidungen

Die meisten frueheren Punkte sind festgelegt (siehe Phase-0-Tabelle: **LM-Studio-Chat**, **ein Modell fuer alle Rollen**, **SQLite**, **LangGraph ohne Cursor-Laufzeit-Dependency** (Reviews wie **Bugbot** optional manuell nebenbei), **SSH ironcrab-prod**, **kein automatisches Deploy**, **Embedding-Default `BAAI/bge-m3`**).

**Optional spaeter:**

1. **Anderes Embedding-Checkpoint** statt Default nur nach konkreter Retrieval-/Qualitaets-Eval (andere Dimension = Index neu bauen).
2. **Postgres** statt SQLite nur dann, wenn der Graph-State nicht mehr nur auf einem PC liegen soll.

---

## Empfohlene Startentscheidung

Fuer den ersten Prototyp (entspricht den getroffenen Festlegungen):

- Orchestrator-Stamm **`C:\Users\Robert Onuk\Langgraph_workflow`**.
- Separate Klone **`Iron_crab`** und **`Iron_crab-eval`**; Impl/Eval in geschuetzte Linien **per PR**.
- Python + LangGraph; **SQLite-Checkpointer** unter `Langgraph_workflow\data\checkpoints`.
- LM Studio Chat unter **`http://100.106.158.144:1234/v1`**, Modell **`qwen3-coder-next`**, fuer **alle** Agenten-Rollen gleichermaßen.
- Zwei Retrieval-Stores (**Impl** vs. **Eval**) auf dem Orchestrator-PC mit **Python-Embeddings** (**Default:** `BAAI/bge-m3`, Dimension **1024**); Supervisor **dual query** wie Abschnitt *Code-Indexierung und Retrieval*.
- Supervisor-SSH fuer **`ironcrab-prod`**, debugging-orientiert; **Deploy nur manuell durch User**.
- **Cursor** weiterhin fuer Editierung/Workspace moeglich; **LangGraph** ohne eingebaute **Cursor Cloud / Bugbot**-API (**Bugbot** optional manuell auf PRs — nicht Teil des Graphen). GitHub CI/Actions wie gewohnt **nebenbei**.
- Erstes Ziel: Handoff erzeugen, Indices fuer einen Teilbaum (sobald Embeddings stehen), lokalen Impl-Agent starten, Reviewer-Finding, Resume testen.

---

## Akzeptanzkriterien fuer den Wechsel

Der Wechsel gilt erst als erfolgreich, wenn:

- Der komplette Scope-Lifecycle laeuft **ohne Cursor Cloud Agents** zur **Orchestrierung** (vorher typisch Rollenwechsel/Cloud API; jetzt **Langgraph_workflow**); proprietäre Review-Bots (**Bugbot** etc.) sind **nicht** in **`Langgraph_workflow`** eingebunden (manuelles Bugbot oder Team-Review optional **ausserhalb**).
- Supervisor, Impl-Agent, Eval-Agent und Reviewer im LangGraph-State nachvollziehbar sind.
- Jeder Agentenlauf checkpointbar und resumable ist.
- Repo-Isolation fuer Impl und Eval technisch erzwungen wird (getrennte GitHub-Repos, getrennte Klone, Eval ohne Impl-Retrieval/Index-Zugriff).
- Lokale Checks und optionale GitHub-CI-Gates maschinenlesbar ausgewertet werden.
- Memory-Suche und Failure-Pattern-Speicherung integriert sind.
- Deploys weiterhin nur nach expliziter User-Freigabe moeglich sind.
- Der Rust-Bot keine LangGraph-/Python-Abhaengigkeit erhalten hat.
- **Retriever/Indices** fuer Impl und Eval getrennt betrieben werden; Eval-Tools koennen den Impl-Index nicht aufrufen; Supervisor kann beide konsumieren oder dual-query ohne Index-Vermischung (**✅** Artefakte + **`search_*`** / Supervisor-Prefetch; Tool-Binding fuer zukuenftige Agents weiterhin durch Code-Review zu wahren).
- Ein **SSH-Debugging-Pfad** zu **`ironcrab-prod`** existiert; **keine unbeaufsichtigten Deploys** durch den Orchestrator; Produktions-Deploys weiterhin nur mit expliziter User-Freigabe.
