use serde::Serialize;
use sqlx::types::{chrono::NaiveDate, BigDecimal, Uuid};

#[derive(sqlx::FromRow, Serialize)]
pub struct Entry {
    id: Uuid,
    accounting_date: NaiveDate,
    currency_date: NaiveDate,
    sender_or_receiver: String,
    address: String,
    source_account: String,
    destination_account: String,
    title: String,
    amount: BigDecimal,
    currency: String,
    reference_number: String,
    operation_type: String,
    category: String,
}
