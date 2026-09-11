# Zeiterfassung (Timecard) mit BMD-Schnittstelle

Lokale Arbeitszeitaufzeichnung für kleine Betriebe in Österreich: Stempeln (Terminal und Portal),
Nichtleistungszeiten, Abwesenheitsanträge, Urlaubs- und Gutstundenkonto, Monatsabschluss mit
PDF (zwei Unterschriftsfelder) und CSV-Export für BMD NTCS Lohn („Abrechnungen importieren“).

Kein Internet nötig. Ein Docker-Host im LAN, erreichbar als `zeiterfassung.local` (Portal und
Verwaltung) und `timecard.local` (Stempelterminal).

Der Umsetzungsplan mit Rechtsrahmen, BMD-Feldregeln und offenen Fragen steht in
[UMSETZUNGSPLAN.md](UMSETZUNGSPLAN.md).

## Aufbau

| Verzeichnis | Inhalt |
|---|---|
| `server/crates/domain` | Rechenkern ohne IO: Wochenmodell, Feiertage (§ 7 ARG), Tagesberechnung, Pausenregel (§ 11 AZG), Warnungen (§ 9, § 12 AZG), Urlaubstage, Abwesenheitsarten mit BMD-Codes. Unit-Tests. |
| `server/crates/server` | axum-API, SQLite (sqlx, Migrationen), Sessions, Stammdaten, Stempeln, Abwesenheiten, Konten, Monatsabschluss, LaTeX-PDF, BMD-CSV. Liefert das Frontend aus. |
| `server/migrations` | Datenbankschema |
| `web` | SvelteKit-Frontend (static adapter), wird in den Server eingebettet |
| `latex` | PDF-Vorlage des Monatsberichts (minijinja + LuaLaTeX) |
| `deploy` | Dockerfile, Compose, Caddyfile, Backup-Skript |
| `infos` | Kundenunterlagen (BMD-Dokus, Arbeitszeitrecht) |

## Entwicklung

Voraussetzungen: Rust ≥ 1.85, Node ≥ 20, eine LaTeX-Installation mit `latexmk` und `lualatex`
(TeX Live oder MiKTeX) für die PDF-Erzeugung.

```bash
cd web && npm install && npm run build      # Frontend nach web/build
cd ../server && cargo run                   # API + Frontend auf http://127.0.0.1:8080
```

Beim ersten Start wird der Benutzer `admin` mit dem Passwort aus `TIMECARD_ADMIN_PASSWORD`
(Standard `admin`) angelegt. Passwort sofort unter „Mein Konto“ ändern.

Umgebungsvariablen: `TIMECARD_DATA_DIR` (Standard `./data`), `TIMECARD_BIND` (Standard `0.0.0.0:8080`),
`TIMECARD_LATEX` (Standard `latexmk`), `RUST_LOG`.

Frontend-Entwicklung mit Hot-Reload: `cd web && npm run dev` (Proxy auf Port 8090, den Server
mit `TIMECARD_BIND=127.0.0.1:8090 cargo run` starten). Im Debug-Build liest der Server `web/build`
zur Laufzeit, im Release-Build ist es einkompiliert.

Tests: `cd server && cargo test`

## Betrieb mit Docker

```bash
TIMECARD_ADMIN_PASSWORD='sicheres-passwort' docker compose -f deploy/compose.yml up -d --build
```

Namensauflösung: `zeiterfassung.local` und `timecard.local` müssen im LAN auf den Host zeigen.
Am einfachsten über DNS-Einträge am Router; Alternativen sind `hosts`-Dateien oder mDNS
(`docker compose --profile mdns up -d`, nur Linux-Host; Android löst `.local` nicht zuverlässig auf).

Backup: `deploy/backup.sh /pfad/zum/backup` (per Cron täglich). Wiederherstellung: Archiv
entpacken, `backup-*.sqlite` als `/data/timecard.sqlite` und den Ordner `exports` in das Volume kopieren.

Die Serveruhr bestimmt die Stempelzeit. Ohne NTP-Zugang die Hardware-Uhr regelmäßig prüfen.

## Fachliche Regeln (Kurzfassung)

- Aufzeichnung minutengenau: Kommen, Gehen, Pause Beginn/Ende (§ 26 AZG). Korrekturen nur durch
  die Verwaltung mit Begründung; Stempelungen werden nie gelöscht, nur storniert (Audit-Log).
- Pause: über 6 h Arbeit mindestens 30 min. Fehlende Pause wird gemeldet; automatischer Abzug nur,
  wenn im Wochenmodell aktiviert (zulässig bei betrieblich festgelegter Pausenlage).
- Warnungen (kein Blockieren): über 10 h, über 12 h, Ruhezeit unter 11 h, Arbeit an Sonn- und
  Feiertagen, offene oder ungültige Stempelfolge.
- Feiertag: Sollzeit gilt als bezahlt (§ 9 ARG); Arbeit am Feiertag zählt zusätzlich.
- Gleitzeitsaldo = Summe der Tagesdifferenzen ab „Zeiterfassung ab“, abzüglich Übertragungen in
  Gutstundentöpfe, zuzüglich manueller Saldo-Buchungen (Anfangswert bei Systemstart).
- Durchrechnung: Am Periodenende schlägt das System die Übertragung des Plus-Saldos (über der
  Übertragsgrenze) in den Gutstundentopf vor (Vollzeit 307, Teilzeit 311, Feiertag 308).
  Zeitausgleich verbraucht den Standard-Topf.
- Urlaub in Arbeitstagen laut Wochenmodell, Feiertage zählen nicht. Halbe Tage und Stunden nur
  nach Freigabe in den Einstellungen, mit Hinweistext. Erstes Urlaubsjahr aliquot. Verbrauch
  nach FIFO vom ältesten Anspruch; Ansprüche verfallen automatisch zwei Jahre nach Ende des
  Urlaubsjahres (§ 4 Abs 5 UrlG, abschaltbar). Ein expliziter Übertrag-Eintrag ersetzt die
  durchgerechneten Vorjahre (für den Systemstart). Urlaubskartei und Urlaubsübersicht als PDF.
- Korrekturanträge: Mitarbeiter beantragen das Nachtragen oder Streichen einer Stempelung mit
  Begründung; die Verwaltung genehmigt oder lehnt ab. Offene Anträge blockieren den Monatsabschluss.
- Krankenstände werden standardmäßig nicht exportiert (ÖGK-Import in BMD), Schalter vorhanden.

## BMD-Export

Datei je Datenmonat unter `data/exports/bmd/`, Trennzeichen `;`, Windows-1252, ohne Kopfzeile
(Kopfzeile per Einstellung). Spalten:

```
MONAT;FIRMA;MA;LOHNART;MENGE;BETRAG;MONAT_A;NLZ_K;NLZ_V;NLZ_B;NLZ_VER;ABM_DIVNLZID
```

- `MONAT` = Abrechnungsmonat = Datenmonat + 1.
- Erster Export eines Monats: Verbuchungsart 3 (neu anlegen/zusammenhängen). Jeder weitere
  Export desselben Monats ist ein Korrekturexport: alle aktuellen Zeilen mit Verbuchungsart 2
  (neu anlegen/ändern, ersetzt überschneidende Einträge in BMD) plus Löschzeilen mit
  Verbuchungsart 1 für Einträge, die im letzten Export enthalten waren und jetzt fehlen
  (gleicher Zeitraum, daher exakt löschbar).
- Abwesenheiten werden im Monat ihres Beginns exportiert, auch wenn sie in den Folgemonat reichen.
- Gutstunden: eine Zeile je Mitarbeiter und Topf mit dem Monatsdelta (Aufbau positiv, Abbau negativ).
- Export erst möglich, wenn alle Monate abgeschlossen sind. Jeder Export wird mit Inhalt,
  SHA-256 und Zeitstempel archiviert.

## Status und offene Punkte

Umgesetzt und lokal getestet: Stammdaten, Wochenmodelle, Terminal und Portal, Abwesenheiten mit
Genehmigung, Korrekturanträge zu Stempelungen, Urlaubs- und Gutstundenkonto mit automatischem
Verfall, Urlaubskartei und Urlaubsübersicht als PDF, Monatsabschluss mit PDF (LuaLaTeX), BMD-CSV
inklusive Korrekturexport, Periodenabschluss, Audit-Log, Backup-Download.

Noch nicht erledigt oder zu klären:

- Docker-Image ist geschrieben, aber ohne Docker auf dem Entwicklungsrechner ungetestet.
- Löschzeilen (Verbuchungsart 1) für Gutstunden-Töpfe und die Spalte `ABM_DIVNLZID` mit dem
  Lohnverrechner in der BMD-Importvorschau prüfen.
- Jahresübersicht der Gleitzeitsalden als Bericht.
- Offene Fragen an den Kunden: siehe [UMSETZUNGSPLAN.md](UMSETZUNGSPLAN.md), Abschnitt 9.
