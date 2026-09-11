# Umsetzungsplan: Zeiterfassung mit BMD-Schnittstelle

Stand: 11.09.2026. Grundlage: `infos/` (BMD-Dokus „Nichtleistungszeiten importieren“ und „Abrechnungen importieren“, E-Mail der Lohnverrechnung `nlz import.txt`, Screenshot der Schnittstellendefinition, Rauch „Arbeitsrecht für Arbeitgeber“ Kap. 30 Arbeitszeitrecht).

---

## 1. Ziel und Rahmen

| Punkt | Entscheidung |
|---|---|
| Zweck | Arbeitszeitaufzeichnung nach § 26 AZG, Nichtleistungszeiten (NLZ), Abwesenheitsanträge, Urlaubsverwaltung, Monatsexport (PDF mit 2 Unterschriften) und BMD-NTCS-Lohnimport (CSV) |
| Betrieb | Rein lokal im LAN, kein Internet. Ein Docker-Host, erreichbar als `timecard.local` und `zeiterfassung.local` |
| Backend | Rust (axum, sqlx, SQLite), PDF über LaTeX im Container |
| Frontend | SPA (SvelteKit, static adapter), vom Rust-Server ausgeliefert; Terminal-Modus (Stempeluhr) und Portal-Modus (Self-Service + Admin) |
| Branding | Unbranded. Name, Logo, Farbe, Fußzeile werden im Adminpanel gepflegt und in UI und PDF verwendet |
| Rollen | `mitarbeiter`, `admin` (später optional `vorgesetzter` für Genehmigungen) |

### Rechtsrahmen, den das Tool abbilden muss (aus dem Buchausschnitt)

- **Aufzeichnungspflicht § 26 AZG:** Beginn und Ende der Tagesarbeitszeit sowie Beginn und Ende der Ruhepausen. Bei fixer schriftlicher Arbeitszeiteinteilung genügt die Aufzeichnung der Abweichungen (§ 26 Abs 5a), bei Telearbeit nur die Dauer (§ 26 Abs 3).
- **Übermittlungsanspruch § 26 Abs 8 AZG:** Arbeitnehmer:innen können monatlich eine Abschrift verlangen. Der Mitarbeiter-PDF-Export deckt das ab.
- **Ruhepause § 11 AZG:** ab mehr als 6 Stunden Tagesarbeitszeit mindestens 30 Minuten (Teilung 2×15 oder 3×10 möglich). Pausen sind keine Arbeitszeit.
- **Höchstgrenzen:** 12 h/Tag, 60 h/Woche, im Schnitt 48 h über 17 Wochen (§ 7, § 9 AZG). Tägliche Ruhezeit 11 h (§ 12 AZG), Wochenendruhe 36 h (§ 3 ARG).
- **Normalarbeitszeit:** 40 h/Woche, 8 h/Tag (§ 3 AZG), KV kann kürzer sein (38,5 / 39). Andere Verteilung bis 9 h (§ 4 Abs 2), 4-Tage-Woche bis 10 h (§ 4 Abs 8).
- **Teilzeit:** Mehrarbeitszuschlag 25 % (§ 19d Abs 3a), entfällt bei Zeitausgleich 1:1 innerhalb des Kalendervierteljahres oder eines anderen festgelegten 3-Monats-Zeitraums (§ 19d Abs 3b). Das ist der „Durchrechnungszeitraum“ der Stammdaten.
- **Überstunden:** 50 % Zuschlag (bzw. 100 % Feiertag laut Kunde). Zeitguthaben bei Ende des Dienstverhältnisses mit 50 % abzurechnen (§ 19e Abs 2).
- **Gleitzeit § 4b AZG:** falls der Kunde Gleitzeit fährt, braucht es eine schriftliche Vereinbarung mit Gleitzeitperiode, Gleitzeitrahmen, Übertragungsgrenzen, fiktiver Normalarbeitszeit. Fehlt ein Element, gelten alle Überschreitungen als Überstunden. Das Tool speichert diese Parameter, ersetzt aber die Vereinbarung nicht.
- **Feiertage § 7 ARG:** 1.1., 6.1., Ostermontag, 1.5., Christi Himmelfahrt, Pfingstmontag, Fronleichnam, 15.8., 26.10., 1.11., 8.12., 25.12., 26.12. Ostern wird lokal berechnet (Gauß). Persönlicher Feiertag § 7a ARG = ein Urlaubstag, den der AN einseitig bestimmt (3 Monate Vorlauf).
- **Aufbewahrung:** verwaltungsstrafrechtlich 1 Jahr, zivilrechtlich sinnvoll 3 Jahre (Verjährung). Das Tool löscht nichts automatisch.

### Prüfung der Aussage „Urlaub nur in Tagen, Stunden eigentlich nicht zulässig“

Der Buchausschnitt behandelt nur das Arbeitszeitrecht und enthält dazu nichts. Websuche (WKO, USP, Fachkommentare):

- Das UrlG bemisst Urlaub in Werktagen bzw. Arbeitstagen; Grundregel ist der Verbrauch ganzer Arbeitstage.
- Ein stundenweiser Verbrauch ist gesetzlich nicht vorgesehen. Der OGH hat ihn nur ausnahmsweise und nur im Interesse des Arbeitnehmers akzeptiert. Halbe Tage werden in der Praxis geduldet, sind aber ebenfalls nicht im Gesetz.
- Die WKO nennt eine „wertneutrale Umrechnung des Urlaubsanspruchs in Stunden“ als Vereinbarungsmöglichkeit mit erhöhtem Verwaltungsaufwand. Eine KV-Umrechnung in Stunden hat der OGH (8 ObA 80/14v) als unwirksam beurteilt.

**Ergebnis:** Die Aussage stimmt im Kern, „nicht zulässig“ ist aber zu hart. Korrekte Formulierung für den Hinweistext im Tool:

> Urlaub ist nach dem Urlaubsgesetz in ganzen Arbeitstagen zu verbrauchen. Ein stundenweiser Verbrauch ist gesetzlich nicht vorgesehen und nur ausnahmsweise auf Wunsch und im Interesse des Arbeitnehmers mit ausdrücklicher Vereinbarung vertretbar. Bitte vor Verwendung rechtlich prüfen.

Umsetzung: Einstellung `urlaubseinheit` = `tage` (Standard) mit Schaltern `halbe_tage_erlauben` und `stunden_erlauben`. Beide Schalter zeigen den Hinweis, `stunden_erlauben` zusätzlich beim Buchen. BMD unterstützt beides über `NLZ_VER` (z. B. 0,5) und `MENGE` (Stunden).

Quellen: [WKO Urlaubsverbrauch](https://www.wko.at/arbeitsrecht/urlaubsverbrauch), [WKO Urlaubsanspruch](https://www.wko.at/arbeitsrecht/urlaubsanspruch), [USP Urlaub](https://www.usp.gv.at/themen/mitarbeiter-und-gesundheit/urlaub-und-arbeitszeit/urlaub.html), [finanzundrecht.at Urlaubsverbrauch](https://finanzundrecht.at/arbeit/urlaubsverbrauch/), [DRdA 2015 zum OGH](https://www.drda.at/drda/2015/357/10/24-Der-OGH-als-Retter-des-Gesetzgebers).

---

## 2. BMD-Schnittstelle (aus den Infos abgeleitet)

### 2.1 Zieldatei

Import über **Abrechnungen importieren** mit der Schnittstelle des Kunden (Definition Nr. 2 „27210390“ laut Screenshot). Spalten in dieser Reihenfolge, Trennzeichen `;`:

```
MONAT;FIRMA;MA;LOHNART;MENGE;BETRAG;MONAT_A;NLZ_K;NLZ_V;NLZ_B;NLZ_VER
```

Zusätzlich muss beim Lohnverrechner die Spalte **`ABM_DIVNLZID`** (Feldbezeichnung „Diverse NLZ“) in die Schnittstelle aufgenommen werden, weil die behördliche Absonderung als diverse NLZ 306 mit `ABM_DIVNLZID = 101` übergeben wird. Alternativ: Import mit Doku-Zeile (Kopfzeile mit Feldkürzeln), dann ist die Spaltenreihenfolge egal. Das ist mit dem Lohnverrechner in Phase 0 abzustimmen.

### 2.2 Feldregeln

| Feld | Regel |
|---|---|
| `MONAT` | Abrechnungsmonat = Monat der Übermittlung = Datenmonat + 1 (September-Daten → 10). Dezember-Daten → 1 des Folgejahres; ob zusätzlich `BRE_LOHNJAHR` nötig ist, in Phase 0 klären |
| `FIRMA` | BMD-Firmennummer aus den Einstellungen (Kunde: 27210390) |
| `MA` | BMD-Mitarbeiternummer aus den Stammdaten (führende Nullen egal) |
| `LOHNART`, `BETRAG`, `MONAT_A` | leer (nur NLZ-Import; keine Aufrollung beim NLZ-Import möglich) |
| `NLZ_K` | dreistellig: Hunderter = Verbuchungsart, Zehner/Einer = NLZ-Typ (siehe 2.3). Standard-Verbuchungsart 3 = neu anlegen/zusammenhängen |
| `NLZ_V`, `NLZ_B` | `TT.MM.JJJJ`. Krank/Unfall darf ohne Bis-Datum geliefert werden (laufende NLZ) |
| `NLZ_VER` | Verwaltungswert (Tage oder Stunden). **Leer lassen**, wenn BMD aus dem Wochenmodell rechnen soll. `0` wird als Zahl 0 importiert, nicht als Fallback (Doku Kap. 6.1). Bei Gutstunden Pflichtwert, Vorzeichen = Aufbau (+) / Abbau (−) |
| `MENGE` | „Bezahlt“-Stunden in Industriezeit (0,5 statt 30 min). Leer lassen außer bei bewusster Übersteuerung (z. B. Urlaub in Stunden) |
| Dezimaltrennzeichen | Komma (österreichisches Excel). Konfigurierbar |
| Zeichensatz | Windows-1252 als Standard, UTF-8 optional. Kein BOM |
| Dateiname | `YYYYMMDDHHmmss_NLZ_<Firma>_<Datenmonat>.csv`, damit BMD bei mehreren Dateien die Reihenfolge einhält |

### 2.3 Mapping der Buchungsarten

| Buchung im Tool | NLZ_K | Zusatz | Anmerkung |
|---|---|---|---|
| Urlaub | 301 | `NLZ_VER` leer; bei halbem Tag tageweise Zeilen mit `NLZ_VER = 0,5` in chronologischer Reihenfolge | Tageweiser Import Pflicht, sobald ein Tag vom Wochenmodell abweicht |
| Pflegeurlaub | 305 | leer, BMD rechnet nach Sollstunden | Selten |
| Zeitausgleich / Gutstunden Vollzeit (50 %) | 307 | `NLZ_V` = 1., `NLZ_B` = letzter des Datenmonats, `NLZ_VER` = Monatsdelta ± | Eine Zeile pro Mitarbeiter und Topf und Monat |
| Gutstunden Teilzeit (25 %) | 311 | wie oben | |
| Gutstunden Feiertagszuschlag (100 %) | 308 | wie oben | |
| Behördliche Absonderung | 306 | `ABM_DIVNLZID = 101` | |
| Persönlicher Feiertag | 315 / 316 (gearbeitet) | | Nur wenn der Kunde ihn nutzt |
| Krankenstand / Arbeitsunfall / Freizeitunfall | 302 / 303 / 314 | **standardmäßig nicht exportiert** | Kommt über ÖGK-Import. Schalter „Krankenstände zum Abgleich mitliefern“, dann entfernt der Lohnverrechner die Zeilen per Makro |
| Arzt | 304 | | Nur wenn gewünscht |

Nicht exportiert werden: Sachbezug (fix in BMD), normale Arbeitszeit (Gehalt läuft in BMD), Zuschlagsberechnung in Geld (macht BMD anhand des Gutstundentopfes).

### 2.4 Beispiel (Septemberdaten, Export im Oktober)

```
MONAT;FIRMA;MA;LOHNART;MENGE;BETRAG;MONAT_A;NLZ_K;NLZ_V;NLZ_B;NLZ_VER;ABM_DIVNLZID
10;27210390;6;;;;;311;01.09.2026;30.09.2026;10;
10;27210390;7;;;;;307;01.09.2026;30.09.2026;-15;
10;27210390;7;;;;;301;14.09.2026;18.09.2026;;
10;27210390;8;;;;;301;21.09.2026;21.09.2026;0,5;
10;27210390;9;;;;;306;03.09.2026;07.09.2026;;101
```

### 2.5 Korrekturen nach Export

- Jeder Export wird mit Hash, Zeitstempel und Zeilen archiviert; der Monat wird gesperrt.
- Nachträgliche Änderung erzeugt einen **Korrekturexport**: Verbuchungsart 1 (löschen, Zeitraum muss exakt stimmen) plus neue Zeile mit 3, oder Verbuchungsart 2 (neu anlegen/ändern, löscht Überschneidungen). Gutstunden-Korrektur als weitere Delta-Zeile.
- Vor dem Export: Validierung auf Überschneidungen gleicher NLZ-Art (BMD verweigert bei Verbuchungsart 0 schon bei einem Kalendertag), fehlende MA-Nummer, offene Stempelungen, nicht genehmigte Anträge.

---

## 3. Architektur

```
┌──────────────── Docker-Host (LAN, offline) ────────────────┐
│  caddy (Reverse Proxy, Host-Routing, optional lokale CA)    │
│    timecard.local      → /terminal  (Stempeluhr, Kiosk)     │
│    zeiterfassung.local → /          (Portal + /admin)       │
│                                                              │
│  timecard-server (Rust/axum)                                 │
│    REST-API  ·  Session-Auth  ·  Rechenkern  ·  Exporte      │
│    eingebettete SPA-Assets (rust-embed)                      │
│    LaTeX-Renderer (latexmk/lualatex im selben Image)         │
│                                                              │
│  Volumes: /data/db.sqlite  /data/exports  /data/backups      │
└──────────────────────────────────────────────────────────────┘
```

### 3.1 Technologie

| Baustein | Wahl | Begründung |
|---|---|---|
| Web-Framework | `axum` + `tower` | Standard, async, gute Middleware |
| DB | SQLite (WAL) über `sqlx` mit Migrationen | Eine Datei, Backup = Dateikopie, für einen Betrieb völlig ausreichend. Postgres wäre über sqlx später möglich |
| Auth | Session-Cookies, `argon2`-Passwörter, PIN für Terminal-Stempeln, Rate-Limit | Kein OAuth möglich (offline) |
| Zeit | `chrono` + `chrono-tz` (Europe/Vienna), Speicherung UTC, Anzeige lokal | Sommerzeit korrekt |
| Templates PDF | LaTeX-Templates mit `minijinja` befüllt, eigene Escape-Funktion für LaTeX-Sonderzeichen | |
| PDF-Engine | TeX Live (`texlive-latex-recommended`, `-latex-extra`, `-lang-german`, `-luatex`) im Image, Aufruf `latexmk -lualatex` in Temp-Verzeichnis mit Timeout | Tectonic lädt Pakete bei Erstnutzung aus dem Netz, daher offline ungeeignet, außer der Bundle-Cache wird ins Image gebacken |
| Frontend | SvelteKit (adapter-static), TypeScript, eigenes kleines CSS | Klein, schnell, keine Runtime-Abhängigkeit im Container |
| CSV | `csv`-Crate + `encoding_rs` für Windows-1252 | |
| Tests | Rust-Unit-Tests für Rechenkern, Golden-Files für CSV und LaTeX-Quelltext, Playwright für UI-Smoke | |
| Namensauflösung | `.local` über mDNS (Avahi im Host oder Router-DNS). Windows/macOS/iOS lösen mDNS nativ auf, Android nicht zuverlässig | Fallback: DNS-Einträge am Router oder `hosts`-Dateien |
| Uhrzeit | Server ohne NTP-Zugang: Hardware-Uhr prüfen, optional lokaler NTP am Router. Stempelzeit kommt immer vom Server | |

### 3.2 Repository-Struktur

```
timecard/
├── server/                 Rust-Workspace
│   ├── crates/domain/      Rechenkern (reine Funktionen, keine IO)
│   ├── crates/db/          sqlx-Modelle, Migrationen
│   ├── crates/export/      BMD-CSV, LaTeX-Rendering
│   └── crates/api/         axum-Handler, Auth, Main
├── web/                    SvelteKit-Frontend
├── latex/                  Templates (monatsbericht.tex.j2, urlaubskartei.tex.j2)
├── deploy/                 Dockerfile, compose.yml, Caddyfile, backup.sh
├── docs/                   Betriebs- und Adminhandbuch
└── infos/                  Kundenunterlagen (bestehend)
```

---

## 4. Datenmodell

| Tabelle | Wesentliche Felder |
|---|---|
| `employees` | id, personalnr (BMD MA), vorname, nachname, eintritt, austritt, aktiv, urlaubsanspruch_tage (25), urlaubsjahr_beginn (Eintritt oder 1.1.), gutstunden_topf (307/311 automatisch aus Vollzeit/Teilzeit, überschreibbar), pin_hash, rolle |
| `work_schedules` | employee_id, gueltig_ab, mo..so Stunden (8/8/8/8/8/0/0), wochenstunden (berechnet), pausenregel, gleitzeit (bool), gleitzeitrahmen_von/bis, kernzeit. Versioniert, damit Modellwechsel sauber sind |
| `settlement_periods` | employee_id, typ (quartal / 3 Monate ab Datum / gleitzeitperiode n Monate), start, uebertrag_max_plus, uebertrag_max_minus. Durchrechnungszeitraum |
| `punches` | id, employee_id, ts_utc, art (kommen, gehen, pause_start, pause_ende), quelle (terminal, portal, admin, import), erfasst_von, storniert_durch. Unveränderlich, Korrekturen nur durch Storno + Neuanlage |
| `day_records` | employee_id, datum, soll_min, ist_min, pause_min, nlz_min, differenz_min, saldo_min, warnungen (json), geschlossen. Abgeleitet, wird bei jeder Änderung neu berechnet |
| `absences` | id, employee_id, art (urlaub, krank, arbeitsunfall, freizeitunfall, pflegeurlaub, zeitausgleich, absonderung, sonderurlaub, pers_feiertag, arzt, unbezahlt), von, bis, einheit (tag/halb/stunden), wert, status (beantragt, genehmigt, abgelehnt, storniert), beantragt_von, entschieden_von, kommentar |
| `vacation_ledger` | employee_id, urlaubsjahr, anspruch, uebertrag, verbrauch, korrektur, rest. Reständerung durch Admin mit Begründung |
| `credit_hours_ledger` | employee_id, monat, topf (307/311/308), aufbau, abbau, saldo, exportiert. Gutstunden |
| `holidays` | datum, name, bundesland_optional, betrieblich (bool). Gesetzliche werden generiert, betriebliche vom Admin |
| `month_closures` | employee_id, monat, geschlossen_am, von, pdf_pfad, pdf_sha256 |
| `export_runs` | id, datenmonat, abrechnungsmonat, datei, sha256, zeilen, art (voll / korrektur), erstellt_von |
| `settings` | Branding, Firmennummer, KV-Normalarbeitszeit (40 / 38,5), Zuschlagsregeln, Pausenautomatik, Rundung, Urlaubseinheit-Schalter, Krank-Export-Schalter, Zeichensatz |
| `audit_log` | wer, wann, was, vorher/nachher (json) |

---

## 5. Rechenkern (crate `domain`)

Reine Funktionen, vollständig testbar ohne Datenbank.

1. **Tagessoll** aus `work_schedules` zum Datum; Feiertag → Soll 0 mit Feiertagsentgelt; genehmigte ganztägige NLZ → NLZ-Stunden = Soll.
2. **Tages-Ist** aus Stempelpaaren; offene Stempelung nach Tagesende → Warnung, kein Ist. Pausen: gestempelte Pausen abziehen; wenn keine gestempelt und Ist > 6 h → automatischer Abzug 30 min (konfigurierbar, nur zulässig, wenn die Pausenlage betrieblich festgelegt ist, sonst Warnung „Pause fehlt“).
3. **Rundung** optional (keine Rundung als Standard, weil die Aufzeichnung minutengenau sein soll).
4. **Saldo** = Σ(Ist + NLZ − Soll) ab Beginn des Durchrechnungszeitraums, plus Übertrag.
5. **Periodenabschluss** am Ende des Durchrechnungszeitraums:
   - Vollzeit: Plus-Saldo über Übertragsgrenze → Gutstunden Topf 307.
   - Teilzeit: Plus-Saldo → Topf 311 (Mehrarbeit 25 %), Stunden über der Vollzeit-Normalarbeitszeit → 307.
   - Arbeit an Feiertagen → 308.
   - Minus-Saldo → Übertrag oder Hinweis (Abzug nur mit Vereinbarung).
   Die Zuordnung ist als Regelsatz in `settings` konfigurierbar, weil sie vom KV abhängt.
6. **Warnungen** (kein Blockieren, nur Anzeige und PDF-Vermerk): > 10 h Tagesarbeitszeit, > 12 h, Ruhezeit < 11 h, Woche > 50 h / 60 h, 17-Wochen-Schnitt > 48 h, Wochenendruhe < 36 h, Pause fehlt, Stempelung ohne Gegenstück.
7. **Urlaub** in Tagen: Verbrauch = Anzahl Arbeitstage laut Wochenmodell im Zeitraum, Feiertage und freie Tage zählen nicht. Halbe Tage und Stunden nur mit den Schaltern aus Abschnitt 1.
8. **Krankenstand** im Urlaub: Urlaub wird ab dem 4. Kalendertag Krankheit unterbrochen (§ 5 UrlG); Admin bucht das manuell, das Tool zeigt den Hinweis.

---

## 6. Funktionen je Rolle

### Mitarbeiter (Portal `zeiterfassung.local`, Terminal `timecard.local`)

- Stempeln: Kommen, Gehen, Pause Start/Ende. Terminal: PIN-Eingabe, große Buttons, zeigt nach dem Stempeln den Tagessaldo. Portal: gleiche Buttons plus Tagesliste.
- NLZ einstempeln: Auswahl der Art beim Stempeln oder nachträglich am Tag (Arztbesuch, Zeitausgleich stundenweise, Dienstreise, Absonderung). Welche Arten Mitarbeiter selbst buchen dürfen, gibt der Admin frei.
- Abwesenheit beantragen: Urlaub, Zeitausgleich, Pflegeurlaub, persönlicher Feiertag, Sonderurlaub. Zeigt Resturlaub und Gutstundensaldo. Krankenstand melden (nur Info an Admin, Buchung macht der Admin).
- Übersicht: Monatskalender, Saldo, Urlaubskonto, Gutstundenkonto, eigene PDF-Monatsberichte herunterladen (§ 26 Abs 8 AZG).
- Korrekturantrag für vergessene Stempelung (Admin genehmigt).

### Admin (`zeiterfassung.local/admin`)

- Stammdaten: Mitarbeiter, Wochenmodell mit Gültig-ab, Durchrechnungszeitraum, Urlaubsanspruch, Urlaubsjahr, BMD-Nummer, PIN zurücksetzen.
- Buchungen einfügen und korrigieren: Krankenstand, Urlaub, Arbeitsunfall, Absonderung, Zeitausgleich, Stempelkorrekturen. Jede Änderung mit Begründung im Audit-Log.
- Anträge genehmigen/ablehnen, Kalenderübersicht aller Abwesenheiten.
- Urlaubsverwaltung: Anspruch, Übertrag, Reständerung, Verfall-Hinweis (2 Jahre nach Ende des Urlaubsjahres), Jahresübersicht.
- Gutstunden: Salden pro Topf, manuelle Korrektur, Auszahlung markieren.
- Feiertage: gesetzliche automatisch, betriebliche ergänzen.
- Monatsabschluss: Prüfliste (offene Stempelungen, unbeantwortete Anträge, Warnungen), Sperren, PDF-Erzeugung einzeln oder als ZIP, BMD-CSV erzeugen, Vorschau, Download, Archiv.
- Einstellungen: Branding, Firmennummer, KV-Normalarbeitszeit, Zuschlagsregeln, Pausenautomatik, Urlaubseinheit-Schalter, Krank-Export-Schalter, Backup auslösen.
- Benutzer: Rollen, Passwort zurücksetzen.

---

## 7. PDF-Monatsbericht (LaTeX)

- A4 quer, Kopf mit Branding, Mitarbeiter, Personalnummer, Monat, Wochenmodell, Durchrechnungszeitraum.
- Tabelle je Kalendertag: Datum, Wochentag, Soll, Kommen, Gehen, Pausen (von–bis), Ist, NLZ-Art und Dauer, Differenz, laufender Saldo, Bemerkung (Warnung, Korrektur, Feiertag).
- Summenblock: Soll, Ist, NLZ nach Art, Saldo Periodenbeginn / Monatsende, Gutstunden je Topf, Urlaub Anspruch / verbraucht / Rest, Krankenstandstage.
- Hinweisblock: Warnungen des Monats im Klartext.
- Zwei Unterschriftsfelder mit Datum: „Arbeitnehmer:in“ und „Arbeitgeber:in / Vorgesetzte:r“. Beschriftungen im Admin änderbar.
- Fußzeile: Erstellt am, Version, Seite x von y, Hash der Datenbasis.
- Technik: `minijinja`-Template → `.tex` → `latexmk -lualatex -interaction=nonstopmode` in Temp-Dir mit 60 s Timeout → PDF nach `/data/exports/<jahr>/<monat>/`. Schriftart im Image (z. B. Source Sans, TeX Gyre). PDF wird nach Monatsabschluss nicht neu erzeugt, außer der Abschluss wird aufgehoben.
- Weitere Berichte: Urlaubskartei pro Jahr, Jahresübersicht Salden, Abwesenheitskalender.

---

## 8. Phasen und Aufwand

Aufwand in Personentagen (PT) für eine erfahrene Person, ohne Puffer.

| Phase | Inhalt | PT |
|---|---|---|
| 0 Klärung | Antworten auf offene Fragen (Abschnitt 9), Schnittstelle beim Lohnverrechner um `ABM_DIVNLZID` ergänzen, Musterdatei mit 3 Testzeilen in der BMD-Importvorschau prüfen, Serverhardware und Terminalgerät festlegen | 2 |
| 1 Grundgerüst | Workspace, Docker-Image (Rust + TeX Live), Caddy mit Host-Routing, SQLite-Migrationen, Auth, Rollen, Stammdaten-CRUD, Branding, Audit-Log, Backup-Skript | 8 |
| 2 Zeiterfassung | Stempeln (Terminal und Portal), Wochenmodelle, Feiertagsgenerator, Rechenkern Soll/Ist/Pause/Saldo mit Tests, Tages- und Monatsansicht, Korrekturen, Warnungen | 10 |
| 3 Abwesenheiten | Antrag/Genehmigung, Kalender, Urlaubsledger in Tagen mit Schaltern und Hinweistext, Admin-Buchungen (Krank, Urlaub, Absonderung), Gutstundenledger, Periodenabschluss mit Regelsatz | 8 |
| 4 Export | Monatsabschluss mit Prüfliste, LaTeX-Monatsbericht, ZIP-Batch, BMD-CSV inkl. Vorschau, Korrekturexport, Archiv; Abnahme mit echtem Testimport beim Lohnverrechner | 7 |
| 5 Betrieb | Härtung (Rate-Limit, Session-Ablauf, CSP), Backup/Restore-Test, Handbuch Admin und Mitarbeiter, Installation vor Ort, Parallelbetrieb ein Monat mit bisherigem System, Schulung | 5 |
| **Summe** | | **40 PT** |

Meilensteine: nach Phase 2 stempelt der Betrieb probeweise; nach Phase 4 erster echter Export in BMD parallel zum alten Verfahren.

---

## 9. Offene Fragen an den Kunden (vor Phase 1)

1. Welcher Kollektivvertrag gilt? Davon hängen Normalarbeitszeit (40 / 38,5 / 39), Zuschläge und Durchrechnungsregeln ab.
2. Gibt es eine schriftliche Gleitzeitvereinbarung? Falls ja: Gleitzeitperiode, Rahmen, Übertragsgrenzen. Falls nein: fixe Arbeitszeiten je Mitarbeiter (dann reicht nach § 26 Abs 5a die Abweichungsaufzeichnung, das Tool zeichnet trotzdem voll auf).
3. Durchrechnungszeitraum pro Mitarbeiter: Kalenderquartal oder anderer 3-Monats-Zeitraum? Gleich für alle?
4. Ist die Pausenlage betrieblich festgelegt (dann automatischer Abzug zulässig) oder muss gestempelt werden?
5. Sollen Krankenstände zum Abgleich in die CSV (Schalter) oder gar nicht?
6. Dezember-Export: `MONAT = 1` des Folgejahres, zusätzlich Lohnjahr nötig? Beim Lohnverrechner klären.
7. Terminal: eigenes Gerät (Tablet im Kiosk-Modus) oder nur Browser am Arbeitsplatz? RFID/NFC gewünscht?
8. Urlaubsjahr: Eintrittsdatum (gesetzlicher Standard) oder Kalenderjahr per Vereinbarung?
9. Bundesland für Landesfeiertage (z. B. 19.3. Josefitag) und betriebliche freie Tage.
10. Wer genehmigt Anträge, nur der Admin oder auch Vorgesetzte?
11. HTTPS im LAN gewünscht (lokale CA, Zertifikat auf Clients verteilen) oder HTTP ausreichend?
12. Backup-Ziel: NAS im LAN, USB, oder nur lokal am Server?

---

## 10. Risiken

| Risiko | Gegenmaßnahme |
|---|---|
| KV-spezifische Zuschlagsregeln passen nicht ins generische Modell | Regelsatz konfigurierbar; BMD rechnet das Geld, das Tool liefert nur Stunden je Topf |
| BMD-Importfehler erst beim Lohnverrechner sichtbar | Musterdatei in Phase 0 testen, Err-Datei-Handling dokumentieren, CSV-Vorschau im Tool |
| `.local` auf Android-Geräten nicht auflösbar | DNS-Einträge am Router als Standardweg, mDNS als Zusatz |
| Serveruhr läuft ohne NTP falsch | Uhrenprüfung im Admin-Dashboard, Hinweis bei Abweichung zur Client-Uhr > 2 min |
| TeX Live macht das Image groß (ca. 1 GB) | Akzeptabel für lokalen Betrieb; Alternative Typst (reines Rust, kein externes Binary) falls Größe stört |
| Gleitzeitvereinbarung fehlt oder unvollständig | Hinweis an Kunden (§ 4b AZG), Tool speichert Parameter, ersetzt Vereinbarung nicht |
| Datenschutz (Arbeitszeitdaten sind personenbezogen) | Rollen, Audit-Log, keine Auswertungen über Dritte, Backup verschlüsselt, Aufbewahrung konfigurierbar |
| Nachträgliche Änderungen nach Export | Monatssperre, Korrekturexport mit Verbuchungsart 1/2, Audit-Log |

---

## 11. Nächste Schritte

1. Offene Fragen (Abschnitt 9) mit Kunde und Lohnverrechner klären.
2. Musterdatei aus Abschnitt 2.4 an den Lohnverrechner zur Importvorschau senden.
3. Phase 1 starten: Repository anlegen, Docker-Image bauen, Stammdaten.
