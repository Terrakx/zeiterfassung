# Design-Briefing: Zeiterfassung (Timecard)

## Was das Produkt ist

Eine Web-App zur Arbeitszeitaufzeichnung für kleine Betriebe in Österreich (5 bis 50 Mitarbeitende). Sie läuft lokal im Firmennetz, ohne Internet, und ist **unbranded**: Der Kunde trägt Firmenname, Logo, Primärfarbe und Fußzeile selbst in den Einstellungen ein. Das Design muss deshalb mit jeder Primärfarbe funktionieren und ohne Logo gut aussehen.

Drei Oberflächen in einer App:

1. **Terminal** (Kiosk): Tablet oder Bildschirm am Eingang. Mitarbeitende tippen Personalnummer und PIN auf einem Ziffernblock und drücken Kommen, Gehen, Pause. Große Touch-Ziele, keine Navigation, automatischer Reset nach 20 Sekunden.
2. **Portal** für Mitarbeitende: Stempeln am Arbeitsplatz, eigene Monatsübersicht, Abwesenheiten beantragen, Korrekturen beantragen, Passwort und PIN ändern.
3. **Verwaltung** (Admin): Tagesübersicht aller Mitarbeitenden, Stammdaten, Anträge genehmigen, Monatsabschluss mit PDF, Export für die Lohnverrechnung, Feiertage, Einstellungen, Protokoll.

## Nutzerinnen und Nutzer

- **Mitarbeitende**: nutzen es täglich 2 bis 4 Mal für wenige Sekunden. Wollen sofort sehen: Bin ich eingestempelt? Wie viel habe ich heute? Wie ist mein Saldo? Wie viel Urlaub bleibt? Viele arbeiten nicht am Schreibtisch; Handy und Tablet sind wichtig.
- **Verwaltung / Büro**: eine Person, nutzt es einmal täglich kurz und einmal im Monat intensiv (Abschluss, Export). Braucht Überblick, Prüflisten, klare Fehlermeldungen und Sicherheit, dass nichts verloren geht. Keine IT-Kenntnisse.

## Tonalität und Gestaltungsprinzipien

- Ruhig, sachlich, vertrauenswürdig. Es geht um Arbeitszeit und Geld, also keine verspielten Elemente, aber auch nicht kalt. Eher „gut geführtes Büro“ als „Startup-Dashboard“.
- Sprache: Deutsch (Österreich), Sie-Form gegenüber Mitarbeitenden, kurze Labels. Fachbegriffe wie Soll, Ist, Saldo, Gutstunden, Nichtleistungszeit bleiben, weil sie in der Lohnverrechnung so heißen.
- Zahlen sind das Wichtigste: Zeiten immer als `h:mm` (8:30, +1:15, −0:45), tabellarische Ziffern, Vorzeichen und Farbe für Plus und Minus. Tage als 25 oder 2,5.
- Zustände statt Dekoration: Status-Badges (eingestempelt, in Pause, abwesend, offen, genehmigt, abgeschlossen), Warnhinweise (Pause fehlt, über 10 Stunden), Blocker (Monat kann nicht abgeschlossen werden, weil …).
- Wenig Farbe: eine Primärfarbe (vom Kunden konfigurierbar, Standard ein gedecktes Blau `#2f6f8f`), dazu semantische Farben für ok, Warnung, Fehler, Information. Neutraler Hintergrund, weiße Karten.
- Alles muss auf 375 px Breite funktionieren und auf einem 1920 px Bildschirm nicht auseinanderfallen (maximale Inhaltsbreite etwa 1100 px).
- Barrierefrei: Kontrast mindestens AA, Fokusringe, Formularlabels, keine Information nur über Farbe.
- Hell als Standard. Ein dunkles Terminal-Thema wäre gut, weil das Gerät oft in einem Eingangsbereich steht.

## Screens im Detail

### Login
Zentrierte Karte mit Logo (optional), Firmenname, Benutzername, Passwort, Button. Link „Zum Stempelterminal“. Fehler: „Benutzername oder Passwort falsch“.

### Terminal (Kiosk, ganzer Bildschirm)
- Oben: Logo/Firmenname, große Uhr mit Sekunden, Datum mit Wochentag.
- Schritt 1: „Personalnummer eingeben“, Ziffernblock 3×4 (1 bis 9, Löschen, 0, OK), Anzeige der Eingabe.
- Schritt 2: „PIN eingeben“, gleiche Tastatur, Punkte statt Ziffern, Abbrechen.
- Schritt 3: Name groß, Zustand („eingestempelt seit 08:02“, „in Pause“, „nicht eingestempelt“), heute gearbeitet, Saldo bis gestern. Dann 1 bis 2 große Buttons: nur die Aktionen, die gerade erlaubt sind (Kommen | Pause + Gehen | Pause Ende + Gehen).
- Schritt 4: Bestätigung, grünes Häkchen, „Gehen 17:03“, heute gearbeitet, Saldo. Nach 6 Sekunden zurück zu Schritt 1.
- Fehler: „Personalnummer oder PIN falsch“, „Sie sind bereits eingestempelt“.
- Touch-Ziele mindestens 64 px, Buttons für Kommen/Gehen mindestens 160×70 px.

### Portal: Stempeln (Startseite)
Zwei Karten nebeneinander (mobil untereinander):
- Links: Datum und Uhrzeit, Status-Badge, die erlaubten Stempel-Buttons, Liste der heutigen Stempelungen (Zeit, Art, Quelle), Warnhinweise des Tages.
- Rechts: sechs Kennzahlen als Kacheln: Ist heute, Soll heute, Gleitzeitsaldo bis gestern (farbig), Resturlaub in Tagen, davon geplant, Gutstunden. Links zu Monatsübersicht und Abwesenheiten.

### Portal: Monatsübersicht
- Kopf: Titel, Monatsnavigation (‹ September 2026 ›), Badge „abgeschlossen“, PDF-Button.
- Sechs Kennzahl-Kacheln: Soll, Ist gesamt, Differenz im Monat, Saldo Monatsbeginn, Saldo Monatsende, Resturlaub.
- Tabelle mit einer Zeile je Kalendertag: Datum, Wochentag, Soll, Kommen, Gehen, Pause, Ist, Abwesenheit (Badge), Differenz (farbig), Hinweise. Wochenenden grau hinterlegt, Feiertage gelb, heute blau. Zukunftstage leer.
- Pro vergangenem Tag ein kleiner Button „Korrektur beantragen“, öffnet einen Dialog: Stempelung nachtragen (Uhrzeit, Art) oder streichen (Auswahl), Begründung. Darunter eine Liste der eigenen Korrekturanträge mit Status.
- Auf dem Handy muss die Tabelle horizontal scrollen oder in eine Kartenansicht pro Tag umschalten.

### Portal: Abwesenheiten
- Links Formular: Art (Urlaub, Zeitausgleich, Pflegeurlaub, persönlicher Feiertag, Sonderurlaub, Arztbesuch, Dienstreise), Von, Bis, optional Einheit (ganze Tage, halber Tag, Stunden) und Kommentar. Bei halben Tagen oder Stunden ein gelber Hinweiskasten mit rechtlichem Text.
- Rechts Urlaubskonto als kleine Tabelle: Anspruch, Übertrag, verbraucht, geplant, Rest; Hinweis „10 Tage aus 2025 verfallen am 31.12.2027“; Gutstunden je Topf; Link „Urlaubskartei als PDF“.
- Unten Tabelle aller Anträge: Art, Von, Bis, Einheit, Status-Badge (offen gelb, genehmigt grün, abgelehnt rot, storniert grau), Kommentar, „Zurückziehen“ bei offenen.

### Portal: Mein Konto
Zwei Karten: Passwort ändern; Terminal-PIN setzen (mit Anzeige der eigenen Personalnummer und ob eine PIN gesetzt ist).

### Verwaltung: Layout
Gleiche Kopfleiste wie das Portal, darunter eine Tab-Leiste: Übersicht, Mitarbeiter, Anträge, Abschluss & Export, Feiertage, Einstellungen, Protokoll.

### Verwaltung: Übersicht heute
Info-Banner „3 offene Anträge“. Tabelle aller aktiven Mitarbeitenden: Nummer, Name (Link), Status-Badge (anwesend, Pause, abwesend, oder Abwesenheitsart wie Urlaub/Krank), Ist heute, Soll, Saldo (farbig), Hinweise-Zähler. Button „Urlaubsübersicht PDF“.

### Verwaltung: Mitarbeiter
Liste (Nummer, Name, Benutzer, Rolle, Eintritt, Austritt, Urlaub/Jahr, aktiv) und Dialog „Neu anlegen“ mit Stammdaten, Passwort, PIN und Wochenmodell (7 kleine Stundenfelder Mo bis So).

### Verwaltung: Mitarbeiter-Detail
Fünf Tabs:
- **Stammdaten**: Formular (Personalnummer, Benutzername, Name, Rolle, Eintritt, Austritt, Zeiterfassung ab, Durchrechnung in Monaten, Urlaubsanspruch, Beginn des Urlaubsjahres, Gutstundentopf), Buttons Speichern und Deaktivieren. Daneben: Zugang (Passwort oder PIN setzen) und Wochenmodelle (Tabelle mit Gültig-ab und Stunden Mo bis So, Formular für ein neues Modell mit Optionen Pausenabzug, Gleitzeit, Kernzeit, Übertragsgrenzen).
- **Zeiten**: Monatsnavigation, Inline-Formular zum Einfügen einer Stempelung (Datum/Uhrzeit, Art, Begründung), dieselbe Monatstabelle wie im Portal, aber mit allen Stempelungen und einem Storno-Knopf je Stempelung.
- **Abwesenheiten**: Formular zum direkten Buchen (auch Krankenstand, Absonderung, unbezahlt), Liste mit Storno.
- **Urlaub**: Konto des laufenden Urlaubsjahres, offene Ansprüche je Jahr, nächster Verfall, Buchungen, Einträge; Formular für Anspruch/Übertrag/Korrektur/Verfall mit Begründung; PDF-Button.
- **Gutstunden**: Gleitzeitsaldo, Tabelle je Topf (Aufbau, Abbau, Saldo), Buchungen, Formular für Korrektur/Auszahlung/Übertrag/Saldo.

### Verwaltung: Anträge
Filter nach Status. Tabelle Abwesenheitsanträge mit Genehmigen/Ablehnen (Ablehnen fragt nach Begründung). Zweite Tabelle Korrekturanträge zu Stempelungen mit denselben Aktionen.

### Verwaltung: Abschluss & Export (der wichtigste Admin-Screen)
- Monatsnavigation, Aktionsleiste: „Alle abschließen + PDFs“, „PDFs als ZIP“, „CSV-Vorschau“, „BMD-CSV erzeugen (Abrechnungsmonat 9)“.
- Tabelle je Mitarbeitendem: Soll, Ist, Differenz, Saldo Ende, Prüfung (rote Blocker-Badges wie „offene Stempelung“, „2 offene Anträge“, „Monat noch nicht vorbei“; gelbes Badge „3 Hinweise“; grünes „ok“), Status (offen / abgeschlossen am Datum), Aktionen (Vorschau, Abschließen | PDF, Aufheben).
- Abschnitt „Durchrechnungsperioden, die in diesem Monat enden“: Saldo, Übertragsgrenze, Vorschlag für die Übertragung in den Gutstundentopf, Button „Periode abschließen“.
- CSV-Vorschau als Tabelle der Exportzeilen mit lesbarer Beschreibung.
- Liste der bisherigen Exporte mit Zeitpunkt, Art (voll / Korrektur), Dateiname, Zeilen, Prüfsumme, Download und Ansehen.

### Verwaltung: Feiertage
Jahresnavigation, Tabelle der gesetzlichen Feiertage (automatisch) plus betriebliche (Badge, entfernbar), Formular zum Hinzufügen.

### Verwaltung: Einstellungen
Gruppen als Karten: Branding (Firmenname, Primärfarbe, Fußzeile, Logo-Upload mit Vorschau), BMD-Schnittstelle (Firmennummer, Zeichensatz, Topfnummern, Schalter für Krankenstände und Kopfzeile), Arbeitszeit (KV-Wochenstunden, Pausenschwelle, Mindestpause), Urlaub (Schalter für halbe Tage, Stunden, automatischen Verfall; Hinweistext), PDF-Bericht (Beschriftung der zwei Unterschriftsfelder), System (Datenbank-Backup herunterladen).

### Verwaltung: Protokoll
Chronologische Tabelle aller Änderungen (Zeit, Wer, Aktion, Ziel), Klick auf eine Zeile klappt Vorher/Nachher als JSON auf.

## Komponenten, die das Design-System braucht

- Kopfleiste mit Brand, Navigation, Benutzername, Abmelden.
- Tab-Leiste (zweite Ebene).
- Karte mit Titel.
- Kennzahl-Kachel (großer Wert, kleines Label, optional Farbe).
- Datentabelle mit rechtsbündigen Zahlen, Zeilenzuständen (Wochenende, Feiertag, heute), horizontalem Scroll auf kleinen Screens.
- Status-Badge in fünf Varianten (neutral, ok, Warnung, Fehler, Info).
- Alert/Hinweiskasten in vier Varianten.
- Buttons: primär, sekundär, gefährlich, groß (Terminal), klein (Tabellenaktionen).
- Formularfelder: Text, Zahl, Datum, Uhrzeit, Datum+Uhrzeit, Select, Checkbox, Radio, Textarea, Farbwähler, Datei-Upload. Label oben, Fehlertext unten.
- Dialog (Modal) mit Formular.
- Monatsnavigation (‹ Monat Jahr ›).
- Ziffernblock und große Aktionsbuttons für das Terminal.
- Leere Zustände („Keine Abwesenheiten.“) und Ladezustand.

## Beispieldaten für Mockups

- Firma: Musterbetrieb GmbH, Primärfarbe #2f6f8f, Fußzeile „Musterbetrieb GmbH · Musterstraße 1 · 1010 Wien“.
- Maria Muster, Nr. 7, Vollzeit 8/8/8/8/8/0/0, Saldo +9:15, Resturlaub 41 Tage, davon geplant 4, heute eingestempelt seit 08:02.
- Josef Teilzeit, Nr. 6, 4/4/4/4/4/0/0, Saldo −2:30, Krankenstand 17. bis 19. August, Resturlaub 35 Tage, 10 Tage verfallen am 31.12.2027.
- Warnungen: „Pause fehlt oder zu kurz (0:00 bei 8:15 Arbeit)“, „Tagesarbeitszeit über 10 h (10:30)“, „Kommen ohne Gehen“.
- Blocker: „offene Stempelung“, „1 offene Korrekturanträge“, „Monat noch nicht vorbei“.
- Export: `20260911193744_NLZ_27210390_2026-08.csv`, 3 Zeilen, Art voll.

## Technischer Rahmen

Das Frontend ist eine SvelteKit-App mit reinem CSS (keine UI-Bibliothek). Das Design sollte sich als CSS-Variablen (Farben, Radius, Abstände, Schrift) und einfache Komponenten umsetzen lassen. Systemschrift oder eine frei verfügbare Schrift, die lokal gebündelt werden kann (kein Internet im Betrieb). Icons sparsam, als Inline-SVG.
