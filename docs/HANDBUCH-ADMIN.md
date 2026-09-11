# Handbuch für die Verwaltung

Die Verwaltung erreichen Sie nach Anmeldung mit einem Admin-Benutzer über „Verwaltung“ in der Kopfleiste.

## Erste Einrichtung

1. **Passwort ändern**: „Mein Konto“, das Startpasswort `admin` ersetzen.
2. **Einstellungen**:
   - Branding: Firmenname, Primärfarbe, Fußzeile, Logo (PNG oder JPG), Terminal hell oder dunkel.
   - BMD-Schnittstelle: Firmennummer, Zeichensatz (Windows-1252 für BMD), Gutstundentöpfe (Vollzeit 307, Teilzeit 311, Feiertag 308), Nummer der diversen NLZ für Absonderung, Schalter für Krankenstände und Kopfzeile.
   - Arbeitszeit: KV-Normalarbeitszeit (40 oder 38,5 Stunden), Pausenschwelle und Mindestpause.
   - Urlaub: halbe Tage, Stunden (mit Hinweistext), automatischer Verfall.
   - PDF: Beschriftung der beiden Unterschriftsfelder.
3. **Feiertage**: gesetzliche Feiertage sind automatisch da. Landesfeiertage oder betriebliche freie Tage ergänzen.
4. **Mitarbeiter anlegen**: Personalnummer (identisch mit der BMD-Mitarbeiternummer), Name, Benutzername, Eintritt, **Zeiterfassung ab** (ab diesem Datum wird Soll und Saldo gerechnet, davor nichts), Durchrechnung in Monaten, Urlaubsanspruch, Beginn des Urlaubsjahres, Passwort, Terminal-PIN, Wochenmodell in Stunden Mo bis So. Das Häkchen **Nimmt an der Zeiterfassung teil** ist standardmäßig gesetzt; für reine Verwaltungskonten (wie den Systemadministrator) aus: keine Stempelung, keine Anträge, nicht in Übersichten, Abschluss und Export.
5. **Altbestände übernehmen** je Mitarbeiter:
   - Gleitzeitsaldo: Tab „Gutstunden“, Buchung Art „Gleitzeitsaldo“, Topf 0, Minuten, Datum = Tag vor „Zeiterfassung ab“.
   - Gutstunden: Buchung Art „Übertrag“ auf den Topf.
   - Resturlaub: Tab „Urlaub“, Eintrag Art „Übertrag“ mit dem Rest aus dem alten System. Ein expliziter Übertrag ersetzt die durchgerechneten Vorjahre.

## Täglich: Übersicht

Anwesenheit aller Mitarbeitenden, heutige Ist- und Sollzeit, Saldo, Hinweise und offene Anträge. Darunter der Abwesenheitskalender des Monats (genehmigt und beantragt). Ein Warnhinweis erscheint, wenn die Serveruhr von der Geräteuhr abweicht.

## Anträge

Zwei Tabellen: Abwesenheitsanträge und Korrekturanträge zu Stempelungen. Genehmigen oder mit Begründung ablehnen. Genehmigte Abwesenheiten lassen sich mit Begründung stornieren, solange der Monat nicht abgeschlossen ist.

## Mitarbeiter-Detail

- **Stammdaten**: Änderungen, Deaktivieren (sperrt Login und Stempeln, Daten bleiben), Passwort und PIN neu setzen, Wochenmodelle mit Gültig-ab (Änderung des Arbeitszeitausmaßes als neues Modell ab Datum), optional Pausenabzug, Gleitzeitrahmen, Kernzeit, Übertragsgrenzen.
- **Zeiten**: Monatsansicht mit allen Stempelungen. Stempelung einfügen (Zeitpunkt, Art, Begründung) oder stornieren (Begründung). Stempelungen werden nie gelöscht; Korrekturen erscheinen im PDF als Nacherfassung.
- **Abwesenheiten**: direkt buchen, auch Krankenstand, Arbeitsunfall, Absonderung, unbezahlter Urlaub. Bei Urlaub warnt das System, wenn der Rest negativ würde.
- **Urlaub**: Konto des Urlaubsjahres, offene Ansprüche je Jahr, nächster Verfall, Einträge (Anspruch, Übertrag, Korrektur, Verfall), Urlaubskartei PDF.
- **Gutstunden**: Gleitzeitsaldo, Töpfe mit Aufbau und Abbau, Buchungen (Korrektur, Auszahlung, Übertrag, Gleitzeitsaldo).

## Monatlich: Abschluss & Export

Reihenfolge nach Monatsende:

1. **Prüfen**: Die Tabelle zeigt je Mitarbeiter Blocker (offene Stempelung, offene Anträge, ungültige Stempelfolge, kein Wochenmodell) und die Anzahl der Hinweise. Blocker beheben: Korrekturen im Mitarbeiter-Detail, Anträge entscheiden.
2. **Durchrechnungsperioden**: Endet in diesem Monat eine Periode, schlägt das System die Übertragung des Plus-Saldos über der Übertragsgrenze in den Gutstundentopf vor. **Periode abschließen** bucht sie; der Wert lässt sich vorher anpassen. Minus-Salden bleiben im Gleitzeitsaldo.
3. **Abschließen**: einzeln mit „Abschließen“ oder „Alle abschließen + PDFs“. Der Monat wird gesperrt, das PDF mit zwei Unterschriftsfeldern erzeugt. „PDFs als ZIP“ lädt alle Berichte. Vorher „Vorschau“ nutzen.
4. **BMD-CSV erzeugen**: erst möglich, wenn alle Monate abgeschlossen sind. „CSV-Vorschau“ zeigt die Zeilen mit Beschreibung. Die Datei wird archiviert (Zeitpunkt, Prüfsumme, Inhalt) und steht zum Download bereit.
5. Datei an die Lohnverrechnung senden (Abrechnungsmonat = Datenmonat + 1).

**Nachträgliche Änderung**: Abschluss mit Begründung aufheben, korrigieren, erneut abschließen, erneut exportieren. Der zweite Export ist ein Korrekturexport mit Verbuchungsart 2 und Löschzeilen für weggefallene Einträge.

Unten auf der Seite: Jahresübersicht der Gleitzeitsalden je Mitarbeiter und Monat, auch als PDF.

## Sicherheit und Betrieb

- Nach fünf Fehlversuchen sind Login (Benutzername und Adresse) oder Terminal-PIN (Personalnummer) 15 Minuten gesperrt; das Protokoll zeigt es.
- **Protokoll**: jede Änderung mit Wer, Wann, Vorher und Nachher. Unveränderlich.
- **Backup**: Einstellungen → System → Datenbank-Backup herunterladen. Für ein vollständiges Backup mit PDFs und CSV-Dateien das Skript `deploy/backup.sh` täglich per Cron laufen lassen.
- Serveruhr: ohne Internet regelmäßig prüfen; die Übersicht warnt bei Abweichung über zwei Minuten.

## Rechtliche Hinweise, die das System abbildet

- Aufzeichnung minutengenau mit Pausen (§ 26 AZG); Abschrift für Mitarbeitende jederzeit als Vorschau-PDF (§ 26 Abs 8 AZG).
- Hinweise bei Tagesarbeitszeit über 10 und 12 Stunden, Wochenarbeitszeit über 50 und 60 Stunden, Ruhezeit unter 11 Stunden, Arbeit an Sonn- und Feiertagen, Stempelungen außerhalb des Gleitzeitrahmens. Hinweise blockieren nichts, sie erscheinen im Monatsbericht.
- Feiertag: Sollzeit gilt als bezahlt, Arbeit am Feiertag zählt zusätzlich (§ 9 ARG).
- Urlaub in Arbeitstagen; Verbrauch vom ältesten Anspruch, Verfall zwei Jahre nach Ende des Urlaubsjahres (§ 4 Abs 5 UrlG). Halbe Tage und Stunden nur mit Freigabe und Hinweistext.
- Gleitzeit setzt eine schriftliche Vereinbarung nach § 4b AZG voraus; das System speichert nur die Parameter.
