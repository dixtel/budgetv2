mod front;
mod migration;
pub mod models;

use std::{
    collections::HashMap,
    error::Error,
    io::{self, Read},
    process,
};

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

#[tokio::main]
async fn main() {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:postgres@localhost/postgres")
        .await
        .unwrap();

    migration::migrate(&pool).await;
    front::start_web_server(&pool).await;
}
