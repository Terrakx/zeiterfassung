-- Teilnahme an der Zeiterfassung. Reine Verwaltungskonten (z. B. der Systemadministrator) stempeln nicht.
ALTER TABLE employees ADD COLUMN stempelt INTEGER NOT NULL DEFAULT 1;
UPDATE employees SET stempelt = 0 WHERE personalnr = '0';
