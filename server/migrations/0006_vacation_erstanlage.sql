-- Erstanlage-Buchungen (Resturlaub zum Erfassungsbeginn) werden über ein eigenes Kennzeichen erkannt,
-- nicht mehr über den Begründungstext. So ersetzt eine erneute Erstanlage genau ihre Vorgänger –
-- auch in einem anderen Urlaubsjahr, wenn „Zeiterfassung ab“ geändert wurde – und manuelle
-- Einträge mit ähnlicher Begründung bleiben unangetastet.
ALTER TABLE vacation_entries ADD COLUMN erstanlage INTEGER NOT NULL DEFAULT 0;
UPDATE vacation_entries SET erstanlage = 1 WHERE grund LIKE 'Erstanlage: Resturlaub %';
