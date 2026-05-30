ALTER TABLE cats ADD COLUMN location TEXT NOT NULL DEFAULT 'adoption center'
    CHECK (location IN ('foster', 'adoption center'));
