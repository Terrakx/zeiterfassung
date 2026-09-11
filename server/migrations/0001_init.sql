-- Grundschema Zeiterfassung. SQLite, alle Zeitstempel UTC als ISO-8601-Text.

CREATE TABLE employees (
    id              INTEGER PRIMARY KEY,
    personalnr      TEXT NOT NULL UNIQUE,          -- BMD-Mitarbeiternummer
    vorname         TEXT NOT NULL,
    nachname        TEXT NOT NULL,
    username        TEXT NOT NULL UNIQUE,
    password_hash   TEXT,                          -- NULL = Login gesperrt
    pin_hash        TEXT,                          -- Terminal-PIN
    rolle           TEXT NOT NULL DEFAULT 'mitarbeiter' CHECK (rolle IN ('mitarbeiter','admin')),
    aktiv           INTEGER NOT NULL DEFAULT 1,
    eintritt        TEXT NOT NULL,                 -- YYYY-MM-DD
    austritt        TEXT,
    urlaubsanspruch_tage REAL NOT NULL DEFAULT 25,
    urlaubsjahr_beginn_mm_dd TEXT NOT NULL DEFAULT '01-01', -- Beginn des Urlaubsjahres, z. B. Eintrittstag
    durchrechnung_monate INTEGER NOT NULL DEFAULT 3,
    durchrechnung_start  TEXT NOT NULL,            -- YYYY-MM-DD, Beginn der Zeiterfassung und der ersten Durchrechnungsperiode
    gutstunden_topf INTEGER,                       -- 307 / 311 / NULL = automatisch
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE work_schedules (
    id           INTEGER PRIMARY KEY,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    gueltig_ab   TEXT NOT NULL,
    mo_min INTEGER NOT NULL DEFAULT 0,
    di_min INTEGER NOT NULL DEFAULT 0,
    mi_min INTEGER NOT NULL DEFAULT 0,
    do_min INTEGER NOT NULL DEFAULT 0,
    fr_min INTEGER NOT NULL DEFAULT 0,
    sa_min INTEGER NOT NULL DEFAULT 0,
    so_min INTEGER NOT NULL DEFAULT 0,
    pause_auto   INTEGER NOT NULL DEFAULT 0,       -- automatischer Pausenabzug
    gleitzeit    INTEGER NOT NULL DEFAULT 0,
    gleitzeit_von TEXT,                            -- HH:MM
    gleitzeit_bis TEXT,
    kernzeit_von  TEXT,
    kernzeit_bis  TEXT,
    uebertrag_max_plus_min  INTEGER,
    uebertrag_max_minus_min INTEGER,
    UNIQUE (employee_id, gueltig_ab)
);

CREATE TABLE punches (
    id           INTEGER PRIMARY KEY,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    ts_utc       TEXT NOT NULL,
    art          TEXT NOT NULL CHECK (art IN ('kommen','gehen','pause_start','pause_ende')),
    quelle       TEXT NOT NULL CHECK (quelle IN ('terminal','portal','admin','import')),
    erfasst_von  INTEGER REFERENCES employees(id),
    kommentar    TEXT,
    storniert_at TEXT,
    storniert_von INTEGER REFERENCES employees(id),
    storno_grund TEXT,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX punches_emp_ts ON punches(employee_id, ts_utc);

CREATE TABLE absences (
    id            INTEGER PRIMARY KEY,
    employee_id   INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    art           TEXT NOT NULL,
    von           TEXT NOT NULL,                   -- YYYY-MM-DD
    bis           TEXT NOT NULL,
    einheit       TEXT NOT NULL DEFAULT 'tag' CHECK (einheit IN ('tag','halber_tag','stunden')),
    wert          REAL,                            -- Stunden bei einheit='stunden', sonst NULL
    status        TEXT NOT NULL DEFAULT 'beantragt' CHECK (status IN ('beantragt','genehmigt','abgelehnt','storniert')),
    kommentar     TEXT,
    beantragt_von INTEGER REFERENCES employees(id),
    beantragt_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    entschieden_von INTEGER REFERENCES employees(id),
    entschieden_at  TEXT,
    entscheidung_kommentar TEXT
);
CREATE INDEX absences_emp_von ON absences(employee_id, von);

CREATE TABLE vacation_entries (
    id           INTEGER PRIMARY KEY,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    urlaubsjahr  TEXT NOT NULL,                    -- Startdatum des Urlaubsjahres YYYY-MM-DD
    art          TEXT NOT NULL CHECK (art IN ('anspruch','uebertrag','korrektur','verfall')),
    tage         REAL NOT NULL,
    grund        TEXT,
    erfasst_von  INTEGER REFERENCES employees(id),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE credit_hours_entries (
    id           INTEGER PRIMARY KEY,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    datum        TEXT NOT NULL,
    topf         INTEGER NOT NULL,                 -- 307 / 311 / 308; 0 = Gleitzeitsaldo
    minuten      INTEGER NOT NULL,                 -- + Aufbau, − Abbau/Auszahlung
    art          TEXT NOT NULL CHECK (art IN ('periodenabschluss','korrektur','auszahlung','uebertrag','saldo')),
    grund        TEXT,
    erfasst_von  INTEGER REFERENCES employees(id),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE custom_holidays (
    datum  TEXT PRIMARY KEY,
    name   TEXT NOT NULL
);

CREATE TABLE month_closures (
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    monat        TEXT NOT NULL,                    -- YYYY-MM
    geschlossen_at TEXT NOT NULL,
    geschlossen_von INTEGER REFERENCES employees(id),
    pdf_pfad     TEXT,
    pdf_sha256   TEXT,
    PRIMARY KEY (employee_id, monat)
);

CREATE TABLE export_runs (
    id            INTEGER PRIMARY KEY,
    datenmonat    TEXT NOT NULL,                   -- YYYY-MM
    abrechnungsmonat INTEGER NOT NULL,
    art           TEXT NOT NULL CHECK (art IN ('voll','korrektur')),
    datei         TEXT NOT NULL,
    sha256        TEXT NOT NULL,
    zeilen        INTEGER NOT NULL,
    erstellt_von  INTEGER REFERENCES employees(id),
    inhalt        TEXT,                            -- JSON der exportierten Zeilen
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL                           -- JSON
);

CREATE TABLE sessions (
    token        TEXT PRIMARY KEY,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    created_at   TEXT NOT NULL,
    expires_at   TEXT NOT NULL,
    terminal     INTEGER NOT NULL DEFAULT 0        -- 1 = Terminal-Sitzung (nur Stempeln)
);

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY,
    ts          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    actor_id    INTEGER,
    aktion      TEXT NOT NULL,
    ziel        TEXT,
    vorher      TEXT,
    nachher     TEXT
);
