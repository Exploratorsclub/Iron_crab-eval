# Supervisor-Kontext — Onboarding-Doku (kein Wiki)

**Datum:** 2026-08-22  
**Entscheidung:** Kein GitHub-Wiki. Onboarding liegt versioniert im Repo (`CONTRIBUTING.md` in beiden Repos).

## Branches für Menschen

- Impl **gemeinsam / Mitentwickler:** `architecture-rebuild`
- Impl **Maintainer aktiv:** `architecture-rebuild-next`
- Eval: `main`

## Warum kein Wiki

- Cloud Agents sehen nur Git, kein Wiki.
- Zwei Repos würden zwei Wikis brauchen.
- Wiki ist nicht PR-reviewbar und driftet.

## Was aktualisiert wurde (Einstieg, nicht jedes historische Plan-File)

- `Iron_crab/CONTRIBUTING.md`, `Iron_crab-eval/CONTRIBUTING.md`
- READMEs, `SPEC_LOCATION.md`, `LEVEL5_EVAL_WORKFLOW.md`, `LEVEL5_CURSOR_SETUP.md`, `LOCAL_SETUP.md`
- Banner in Impl-`INVARIANTS.md` und Eval-`TARGET_ARCHITECTURE.md` (SSOT / aktueller Betriebsstand)
- Eval `AGENTS.md`: schlankes vs. volles Gate

Nicht angefasst: `DEFINITION_OF_DONE.md` (historische Umbau-Checkliste), `docs/plans/*`, `docs/supervisor/handoff_*`, Produktions-Runbook (Betrieb bleibt beim Maintainer).
