CREATE TABLE IF NOT EXISTS account_reference (
    account_id          UUID NOT NULL REFERENCES account(id),
    reference           TEXT NOT NULL
)
