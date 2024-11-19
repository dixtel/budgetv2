use super::AppMessage;
use crate::front::{components::table, AppState};
use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    response::Response,
    Router,
};
use serde::Serialize;
use sqlx::prelude::*;
use uuid::Uuid;

pub fn new_router() -> Router<AppState> {
    Router::new().route("/", axum::routing::get(get))
}

#[axum::debug_handler]
async fn get(
    State(s): State<AppState>,
    Query(q): Query<table::Query>,
) -> Result<Response, AppMessage> {
    let q = q.noramlize();

    dbg!(q.clone());

    #[derive(sqlx::FromRow, Default, Serialize)]
    struct Record {
        id: Uuid,
        name: String,
        account_id: Uuid,
        reference: String,
    }

    let count: i64 = sqlx::query(
        r#"
        SELECT count(*)
        FROM account
        "#,
    )
    .fetch_one(&s.p)
    .await
    .map_err(|err| AppMessage::new_error_notification(anyhow!(err), &s))?
    .try_get(0)
    .map_err(|err| AppMessage::new_error_notification(anyhow!(err), &s))?;

    let records = sqlx::query_as::<_, Record>(
        r#"
        SELECT 
            id,
            name,
            account_id,
            reference
        FROM account 
        JOIN account_reference ON id = account_id
        LIMIT $1
        OFFSET $2 
        "#,
    )
    .bind(q.limit())
    .bind(q.offset())
    .fetch_all(&s.p)
    .await
    .map_err(|err| AppMessage::new_error_notification(anyhow!(err), &s))?;

    let table = table::TableComponent::<Record>::new(records, count, "/api/accounts", q)
        .map_err(|err| AppMessage::new_error_notification(anyhow!(err), &s))?;

    #[derive(Serialize)]
    struct Ctx {
        data: table::TableComponent<Record>,
    }

    let ctx = Ctx { data: table };

    let ret =
        s.t.render("component.table.hbs", &ctx)
            .map_err(|err| AppMessage::new_error_notification(anyhow!(err), &s))?;
    Ok(ret)
}
