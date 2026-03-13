# Handoff: Pool Discovery & Bootstrap Tests (verworfen)

## Status

Dieses Handoff ist **obsolet** und soll **nicht** mehr ausgefuehrt werden.

Die darin beschriebenen A.37-A.40-Tests wurden lokal im Eval-Repo bereits wieder entfernt, weil der Ansatz ueberwiegend aus Source-Code-Scans, Regex-Checks und String-Matching gegen das Impl-Repo bestand und damit keine belastbaren Eval-Tests darstellte.

## Grund fuer die Verwerfung

- keine echten Verhaltens- oder Blackbox-Tests
- starke Kopplung an Implementierungsdetails des Impl-Repos
- hohe Gefahr von Schein-Sicherheit durch Text-Matching statt Laufzeitverhalten

## Aktueller Umgang

- A.37-A.40 gelten derzeit **nicht** als aktive Eval-Invarianten
- dieses Handoff dient nur noch als historische Referenz
- falls spaeter neue Eval-Tests fuer Bug #32/#33 entstehen, muessen diese verhaltensorientiert formuliert werden
