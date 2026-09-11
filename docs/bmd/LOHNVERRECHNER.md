# Abstimmung mit der Lohnverrechnung (BMD NTCS)

Die Zeiterfassung erzeugt monatlich eine CSV-Datei für **LOHN → Weitere Funktionen → Import und
Export → Abrechnungen importieren** mit der bestehenden Schnittstelle (Definition 2, „27210390“).

## Bitte prüfen (Importvorschau mit den Musterdateien in diesem Ordner)

1. **Spalte `ABM_DIVNLZID` ergänzen.** Die Schnittstelle hat derzeit 11 Spalten
   (MONAT … NLZ_VER). Für die behördliche Absonderung (NLZ 306) muss die Nummer der diversen
   NLZ (101) mitgegeben werden. Bitte die Spalte „Diverse NLZ“ (`ABM_DIVNLZID`) als 12. Spalte in
   die Schnittstelle aufnehmen. Alternative: Import mit Doku-Zeile aktivieren („Erste Zeile
   ignorieren“ = Ja, Doku-Zeile mit den Feldnamen aus `Musterdatei_NLZ_2026-09_mit_Kopfzeile.csv`),
   dann ist die Spaltenreihenfolge egal. Wir liefern wahlweise mit oder ohne Kopfzeile.
2. **Musterdatei `Musterdatei_NLZ_2026-09.csv`** in der Importvorschau öffnen. Erwartete Zeilen:
   - Mitarbeiter 6: Gutstunden Topf 311, +10 Stunden für September.
   - Mitarbeiter 7: Gutstunden Topf 307, −15 Stunden; Urlaub 14.–18.9. (5 Tage laut Wochenmodell).
   - Mitarbeiter 8: halber Urlaubstag am 21.9. (`NLZ_VER` = 0,5, tageweise Zeile), ganzer Tag am 22.9.
   - Mitarbeiter 9: Absonderung 3.–7.9. mit DivNLZ 101; Pflegeurlaub am 10.9.
3. **Korrekturdatei `Musterdatei_NLZ_2026-09_Korrektur.csv`** prüfen. Wenn nach dem ersten
   Import etwas geändert wird, senden wir eine zweite Datei: Verbuchungsart **1** (löschen) für
   weggefallene Einträge mit exakt gleichem Zeitraum, Verbuchungsart **2** (neu anlegen/ändern)
   für alle aktuellen Einträge. Bitte bestätigen, dass das Löschen einer Gutstunden-Zeile
   (Kennzeichen 107/111) über den Zeitraum 1.–30. funktioniert, oder ob Gutstunden-Korrekturen
   als zusätzliche Delta-Zeile (z. B. +15) geliefert werden sollen.
4. **Dezember-Daten**: werden mit `MONAT = 1` geliefert. Ist zusätzlich `BRE_LOHNJAHR` nötig,
   damit der Import ins Folgejahr geht?
5. **Krankenstände** liefern wir standardmäßig nicht (ÖGK-Import). Auf Wunsch schalten wir sie
   zum Abgleich ein (302/303/314); sie wären dann per Makro vor dem Import zu entfernen.
6. **Parameter „Verwaltung/Bezahlt bei Zeiträumen“**: Wir lassen `MENGE` und `NLZ_VER` leer,
   damit BMD nach Wochenmodell rechnet, und liefern abweichende Tage (halbe Tage, Stunden) als
   eigene Tageszeilen. Der Parameter kann daher auf „ignorieren“ bleiben.

## Dateiformat

- Trennzeichen `;`, Zeichensatz Windows-1252, Zeilenende CRLF, Dezimaltrennzeichen Komma.
- Dateiname `JJJJMMTThhmmss_NLZ_<Firma>_<Datenmonat>.csv`, damit bei mehreren Dateien die
  Reihenfolge stimmt.
- `MONAT` = Abrechnungsmonat = Datenmonat + 1 (Septemberdaten → 10).
- Abwesenheiten stehen in der Datei des Monats, in dem sie beginnen, auch wenn sie in den
  Folgemonat reichen.
