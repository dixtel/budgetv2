mod front;
mod migration;
pub mod models;

use env_logger::Env;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

#[tokio::main]
async fn main() {
    env_logger::try_init_from_env(Env::default().default_filter_or("budgetv2=debug")).unwrap();
    log::warn!("This is an example message.");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:postgres@localhost/postgres")
        .await
        .unwrap();

    migration::migrate(&pool).await;
    front::start_web_server(&pool).await;
}
