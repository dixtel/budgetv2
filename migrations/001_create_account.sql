CREATE TABLE IF NOT EXISTS account (
    id                  UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    name                TEXT NOT NULL
)
