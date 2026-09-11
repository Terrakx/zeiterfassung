-- Korrekturanträge der Mitarbeiter zu Stempelungen (vergessene oder falsche Stempelung).

CREATE TABLE punch_requests (
    id            INTEGER PRIMARY KEY,
    employee_id   INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    typ           TEXT NOT NULL CHECK (typ IN ('einfuegen','stornieren')),
    punch_id      INTEGER REFERENCES punches(id),   -- bei stornieren
    datum         TEXT NOT NULL,                    -- YYYY-MM-DD (Lokaldatum)
    zeit          TEXT,                             -- HH:MM bei einfuegen
    art           TEXT CHECK (art IN ('kommen','gehen','pause_start','pause_ende')),
    begruendung   TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'beantragt' CHECK (status IN ('beantragt','genehmigt','abgelehnt','storniert')),
    beantragt_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    entschieden_von INTEGER REFERENCES employees(id),
    entschieden_at  TEXT,
    entscheidung_kommentar TEXT
);
CREATE INDEX punch_requests_emp ON punch_requests(employee_id, status);
