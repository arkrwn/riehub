-- 0001_init: records, audit, counters, usage, leases, schema_version
-- (storage spec, sections 2 to 3). All further migrations move
-- forward only.

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS records (
    owner      TEXT    NOT NULL,
    kind       TEXT    NOT NULL,
    id         TEXT    NOT NULL,
    parent     TEXT,
    lookup     TEXT,
    status     TEXT,
    sort       INTEGER NOT NULL DEFAULT 0,
    meta       TEXT    NOT NULL DEFAULT '{}',
    data       BLOB,
    version    INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (owner, kind, id)
);

CREATE UNIQUE INDEX IF NOT EXISTS records_lookup
    ON records (owner, kind, lookup)
    WHERE lookup IS NOT NULL;

CREATE INDEX IF NOT EXISTS records_list
    ON records (owner, kind, parent, status, sort, id);

CREATE TABLE IF NOT EXISTS audit (
    seq   INTEGER PRIMARY KEY,
    ts    INTEGER NOT NULL,
    kind  TEXT    NOT NULL,
    event TEXT    NOT NULL,
    mac   BLOB    NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_head (
    id  INTEGER PRIMARY KEY CHECK (id = 1),
    seq INTEGER NOT NULL,
    mac BLOB    NOT NULL
);

CREATE TABLE IF NOT EXISTS counters (
    owner      TEXT    NOT NULL,
    name       TEXT    NOT NULL,
    window     INTEGER NOT NULL,
    value      INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    PRIMARY KEY (owner, name, window)
);

CREATE TABLE IF NOT EXISTS usage (
    ts           INTEGER NOT NULL,
    owner        TEXT    NOT NULL,
    user_id      TEXT,
    team         TEXT,
    hub_key      TEXT,
    project      TEXT,
    provider_kind TEXT,
    model        TEXT,
    status       TEXT,
    tokens_in    INTEGER NOT NULL,
    tokens_out   INTEGER NOT NULL,
    cost_micros  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS usage_query
    ON usage (owner, ts, status, model, user_id);

CREATE TABLE IF NOT EXISTS usage_buckets (
    owner        TEXT    NOT NULL,
    bucket_start INTEGER NOT NULL,
    groups       TEXT    NOT NULL,
    requests     INTEGER NOT NULL,
    tokens_in    INTEGER NOT NULL,
    tokens_out   INTEGER NOT NULL,
    cost_micros  INTEGER NOT NULL,
    PRIMARY KEY (owner, bucket_start, groups)
);

CREATE TABLE IF NOT EXISTS leases (
    name       TEXT PRIMARY KEY,
    holder     TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
