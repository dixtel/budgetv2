CREATE TABLE IF NOT EXISTS entry (
    id                  UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    accounting_date     DATE NOT NULL,
    currency_date       DATE NOT NULL,
    sender_or_receiver  TEXT NOT NULL,
    address             TEXT NOT NULL,
    source_account      TEXT NOT NULL,
    destination_account TEXT NOT NULL,
    title               TEXT NOT NULL,
    amount              NUMERIC NOT NULL,
    currency            TEXT NOT NULL,
    reference_number    TEXT NOT NULL UNIQUE,
    operation_type      TEXT NOT NULL,
    category            TEXT NOT NULL
)
