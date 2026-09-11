-- Korrekturanträge für einen ganzen Tag: gewünschte Stempelfolge als JSON in "payload".
CREATE TABLE punch_requests_new (
    id            INTEGER PRIMARY KEY,
    employee_id   INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    typ           TEXT NOT NULL CHECK (typ IN ('einfuegen','stornieren','tag')),
    punch_id      INTEGER REFERENCES punches(id),
    datum         TEXT NOT NULL,
    zeit          TEXT,
    art           TEXT CHECK (art IN ('kommen','gehen','pause_start','pause_ende')),
    payload       TEXT,                             -- JSON: [{"zeit":"08:00","art":"kommen"}, ...]
    begruendung   TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'beantragt' CHECK (status IN ('beantragt','genehmigt','abgelehnt','storniert')),
    beantragt_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    entschieden_von INTEGER REFERENCES employees(id),
    entschieden_at  TEXT,
    entscheidung_kommentar TEXT
);
INSERT INTO punch_requests_new (id, employee_id, typ, punch_id, datum, zeit, art, begruendung, status, beantragt_at, entschieden_von, entschieden_at, entscheidung_kommentar)
    SELECT id, employee_id, typ, punch_id, datum, zeit, art, begruendung, status, beantragt_at, entschieden_von, entschieden_at, entscheidung_kommentar FROM punch_requests;
DROP TABLE punch_requests;
ALTER TABLE punch_requests_new RENAME TO punch_requests;
CREATE INDEX punch_requests_emp ON punch_requests(employee_id, status);
