CREATE TABLE cats_new (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL CHECK (color IN ('green', 'orange', 'blue')),
    notes       TEXT NOT NULL DEFAULT '',
    food_notes  TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL,
    location    TEXT NOT NULL DEFAULT 'adoption center'
        CHECK (location IN ('foster', 'adoption center'))
);

INSERT INTO cats_new (id, name, color, notes, food_notes, updated_at, location)
SELECT id, name, CASE color WHEN 'yellow' THEN 'orange' ELSE color END,
       notes, food_notes, updated_at, location
FROM cats;

DROP TABLE cats;
ALTER TABLE cats_new RENAME TO cats;
