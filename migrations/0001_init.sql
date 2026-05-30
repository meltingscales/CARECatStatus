CREATE TABLE IF NOT EXISTS cats (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL CHECK (color IN ('green', 'yellow', 'blue')),
    notes       TEXT NOT NULL DEFAULT '',
    food_notes  TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL
);
