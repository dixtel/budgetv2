pub mod accounts;
pub mod components;
pub mod template;

use std::{
    collections::HashMap,
    error::Error,
    io::{self, Read},
};

use anyhow::Context;
use axum::{
    extract::{connect_info::MockConnectInfo, Multipart, Query, State},
    http::{status, Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response, Result},
    routing::{get, post},
    Router, ServiceExt,
};
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{format, Datelike, DurationRound, TimeDelta, TimeZone};
use log::{warn, Record};
use serde::{Deserialize, Serialize};
use sqlx::{prelude::*, types, Pool, Postgres};
use tower_http::services::ServeDir;

use crate::models;

const MAIN_ACCOUNT_ID: &'static str = "1e7a4379-4fd5-45df-ba1b-fd6f3fc34717";

#[derive(Clone)]
pub struct AppState {
    p: Pool<Postgres>,
    t: template::Template,
}

pub async fn start_web_server(p: &Pool<Postgres>) {
    log::info!("loading templates...");
    let t = template::Template::new();

    // build our application with a single route
    let app = Router::new()
        .route("/", get(index))
        .route("/details", get(details))
        .route("/expenses", get(expenses))
        .route("/api/entry", get(api_entry))
        .route("/api/expenses", get(api_expenses))
        .route("/api/upload", post(api_upload))
        .nest("/accounts", accounts::new_router())
        .nest("/api/accounts", accounts::api::new_router())
        .nest_service("/public", ServeDir::new("./src/front/public"))
        .with_state(AppState { p: p.clone(), t });

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    log::info!("open website at http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn index(State(s): State<AppState>) -> Response {
    #[derive(sqlx::FromRow)]
    struct Result {
        goal: Option<types::BigDecimal>,
    }

    let res = sqlx::query_as::<_, Result>(
        "SELECT SUM(amount) as goal FROM entry WHERE EXTRACT(month from NOW()) = EXTRACT(month from accounting_date)",
    )
    .fetch_one(&s.p)
    .await
    .unwrap();

    #[derive(Serialize)]
    struct Ctx {
        goal: BigDecimal,
    }

    s.t.render(
        "index.hbs",
        &Ctx {
            goal: res.goal.unwrap_or_default(),
        },
    )
    .unwrap()
}

#[derive(Deserialize)]
struct DetailsQuery {
    entry_id: String,
}

async fn details(State(s): State<AppState>, Query(q): Query<DetailsQuery>) -> Response {
    let entry: models::Entry =
        sqlx::query_as::<_, models::Entry>("SELECT * FROM entry WHERE id = $1::uuid LIMIT 1")
            .bind(q.entry_id)
            .fetch_one(&s.p)
            .await
            .unwrap();

    #[derive(Serialize)]
    struct Ctx {
        entry: models::Entry,
    }

    s.t.render("entry_details.hbs", &Ctx { entry }).unwrap()
}

async fn expenses(State(s): State<AppState>) -> Response {
    #[derive(Serialize)]
    struct Ctx {
        years: Vec<u32>,
        current_year: u32,
        current_month: u32,
    }

    #[derive(sqlx::FromRow, Serialize, Debug)]
    struct Record {
        accounting_date: chrono::NaiveDate,
    }
    let r1: Record = sqlx::query_as(
        r#"
        SELECT accounting_date
        FROM entry
        WHERE
            source_account IN (
                SELECT reference FROM account_reference WHERE account_id::text = $1
            ) AND
            amount < 0
        ORDER BY accounting_date ASC
        LIMIT 1
        "#,
    )
    .bind(MAIN_ACCOUNT_ID)
    .fetch_one(&s.p)
    .await
    .unwrap();

    let r2: Record = sqlx::query_as(
        r#"
        SELECT accounting_date
        FROM entry
        WHERE
            source_account IN (
                SELECT reference FROM account_reference WHERE account_id::text = $1
            ) AND
            amount < 0
        ORDER BY accounting_date DESC
        LIMIT 1
        "#,
    )
    .bind(MAIN_ACCOUNT_ID)
    .fetch_one(&s.p)
    .await
    .unwrap();

    let current_year = chrono::Local::now().year() as u32;
    let current_month = chrono::Local::now().month();
    let (_, year1) = r1.accounting_date.year_ce();
    let (_, year2) = r2.accounting_date.year_ce();

    s.t.render(
        "expenses.hbs",
        &Ctx {
            years: (year1..=year2).collect(),
            current_year,
            current_month,
        },
    )
    .unwrap()
}

#[derive(Deserialize)]
struct ExpensesQuery {
    max_elements: Option<u32>,
    month: Option<u32>,
    year: Option<u32>,
}

async fn api_expenses(State(s): State<AppState>, Query(q): Query<ExpensesQuery>) -> Response {
    let year = q.year.unwrap_or_else(|| chrono::Local::now().year() as u32);
    let month = q
        .month
        .unwrap_or_else(|| chrono::Local::now().month() as u32);

    let r1 = chrono::NaiveDate::from_ymd_opt(year as i32, month, 1)
        .unwrap_or(chrono::Local::now().date_naive().with_day(1).unwrap());

    let (_, year) = r1.year_ce();
    let r2 = r1.with_month(r1.month0() + 2).unwrap_or(
        r1.with_year((year + 1) as i32)
            .unwrap()
            .with_month(1)
            .unwrap(),
    );

    #[derive(sqlx::FromRow, Serialize, Debug)]
    struct Record {
        category: String,
        amount: BigDecimal,
    }
    let mut records: Vec<Record> = sqlx::query_as(
        r#"
        SELECT 
            category,
            SUM(amount) AS amount
        FROM entry
        WHERE 
            source_account IN (
                SELECT reference FROM account_reference WHERE account_id::text = $1
            ) AND
            accounting_date >= $2::date AND
            accounting_date < $3::date AND
            amount < 0
        GROUP BY category
        ORDER BY amount ASC
        "#,
    )
    .bind(MAIN_ACCOUNT_ID)
    .bind(r1.format("%Y-%m-%d").to_string())
    .bind(r2.format("%Y-%m-%d").to_string())
    .fetch_all(&s.p)
    .await
    .unwrap();

    if let Some(m) = q.max_elements {
        if records.len() > m as usize {
            records.truncate(m as usize);
        }
    }

    #[derive(Serialize)]
    struct Element {
        amount: BigDecimal,
        category: String,
        height_ratio: String, // from 0 to 100
    }

    #[derive(Serialize)]
    struct Ctx {
        expenses: Vec<Element>,
        date_range: String,
    }

    let max = records.first().map(|r| r.amount.clone().abs());

    s.t.render(
        "api.expenses.hbs",
        &Ctx {
            expenses: records
                .iter()
                .map(|r| Element {
                    amount: r.amount.clone(),
                    category: r.category.clone(),
                    height_ratio: max.clone().map_or(format!("0"), |m| {
                        (r.amount.clone().abs() / m * BigDecimal::from_i32(100).unwrap())
                            .round(0)
                            .to_string()
                    }),
                })
                .collect(),
            date_range: format!(
                "{} - {}",
                r1.format("%Y-%m-%d").to_string(),
                r2.format("%Y-%m-%d").to_string()
            ),
        },
    )
    .unwrap()
}

#[derive(Deserialize)]
struct EntryQuery {
    page: Option<u32>,
}

#[axum::debug_handler]
async fn api_entry(State(s): State<AppState>, Query(query): Query<EntryQuery>) -> Response {
    let mut current_page = query.page.unwrap_or(1);

    let count: i64 = sqlx::query("SELECT COUNT(*) FROM entry")
        .fetch_one(&s.p)
        .await
        .unwrap()
        .try_get(0)
        .unwrap();

    let max_page = (count as f64 / 10.0).ceil() as u32;

    if current_page == 0 {
        current_page = 1;
    } else if current_page >= max_page && max_page > 0 {
        current_page = max_page
    }

    let entries: Vec<models::Entry> = sqlx::query_as::<_, models::Entry>(
        "SELECT * FROM entry ORDER BY accounting_date DESC LIMIT 10 OFFSET $1",
    )
    .bind(((current_page - 1) * 10) as i64)
    .fetch_all(&s.p)
    .await
    .unwrap();

    #[derive(Serialize, Default)]
    struct Pagination {
        page: u32,
        is_current: bool,
        link: String,
    }

    #[derive(Serialize, Default)]
    struct Ctx {
        entries: Vec<models::Entry>,
        pagination: Vec<Pagination>,
        first_page: Option<String>,
        last_page: Option<Pagination>,
        previous_page: Option<String>,
        next_page: Option<String>,
    }

    let mut ctx = Ctx::default();
    ctx.entries = entries;
    ctx.pagination = (current_page as i32 - 3..=current_page as i32 + 3)
        .filter(|p| *p > 0)
        .filter(|p| *p <= max_page as i32)
        .map(|p| Pagination {
            page: p as u32,
            is_current: p == current_page as i32,
            link: format!("/api/entry?page={}", p),
        })
        .collect();

    if ctx.pagination.len() > 0 {
        ctx.last_page = if ctx.pagination.last().unwrap().page == max_page {
            None
        } else {
            Some(Pagination {
                page: max_page,
                is_current: false,
                link: format!("/api/entry?page={}", max_page),
            })
        };
        ctx.first_page = if ctx.pagination.first().unwrap().page == 1 {
            None
        } else {
            Some(format!("/api/entry?page=1"))
        };

        ctx.next_page = if current_page == max_page {
            None
        } else {
            Some(format!("/api/entry?page={}", current_page + 1))
        };
        ctx.previous_page = if current_page == 1 {
            None
        } else {
            Some(format!("/api/entry?page={}", current_page - 1))
        };
    }

    s.t.render("entry.hbs", &ctx).unwrap()
}

struct HtmlTemplate {
    name: String,
}

impl IntoResponse for HtmlTemplate {
    fn into_response(self) -> Response {
        let path = format!("./src/front/public/{}", &self.name);
        let mut handlebars = handlebars::Handlebars::new();
        handlebars.register_template_file(&path, &path).unwrap();

        match handlebars.render(&path, &format!("")) {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}

// todo: this need to be refactored because data was inseeted incorectly, migrate db again
async fn load(p: &Pool<Postgres>, s: impl io::Read) -> anyhow::Result<()> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(s);

    let mut count = 0;

    for result in rdr.records() {
        let record = result?;
        let insert_query = format!(
            r#"INSERT INTO entry (
                 accounting_date,
                 currency_date,
                 sender_or_receiver,
                 address,
                 source_account,
                 destination_account,
                 title,
                 amount,
                 currency, 
                 reference_number,
                 operation_type,
                 category
            ) VALUES (
                to_date($1, 'DD.MM.YYYY'),
                to_date($2, 'DD.MM.YYYY'),
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12
            );"#,
        );

        let amount = record[7]
            .replace(",", ".")
            .replace(" ", "")
            .parse::<f64>()
            .with_context(|| format!("value: {}", &record[7]))?;

        let err = match sqlx::query(&insert_query)
            .bind(&record[0])
            .bind(&record[1])
            .bind(&record[2])
            .bind(&record[3])
            .bind(&record[4])
            .bind(&record[5])
            .bind(&record[6])
            .bind(amount)
            .bind(&record[8])
            .bind(&record[9])
            .bind(&record[10])
            .bind(&record[11])
            .execute(p)
            .await
        {
            Ok(_) => {
                count += 1;
                Ok(())
            }
            Err(err) => {
                if let Some(err) = err.as_database_error() {
                    if err.is_unique_violation() {
                        log::warn!(
                            "cannnt load entry: violation for entry {:?}: {}",
                            record,
                            err
                        );
                        continue;
                    }
                }

                log::error!("cannot insert: {}: {}", insert_query, err);

                return Err(anyhow::anyhow!(err));
            }
        };

        if let Err(err) = err {
            return err;
        }
    }

    log::info!("{} records were loaded to db", count);

    Ok(())
}

#[axum_macros::debug_handler]
async fn api_upload(
    State(s): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, (StatusCode, String)> {
    while let Ok(Some(m)) = multipart.next_field().await {
        let file_name = m.file_name().unwrap();
        log::info!("file_name={}", file_name);

        let name = m.name().unwrap();
        log::info!("name={}", name);

        let bytes = m.bytes().await.unwrap();

        match load(&s.p, bytes.as_ref()).await {
            Ok(_) => (),
            Err(err) => log::error!("cannot load file: {}", err),
        }
    }

    return Err((StatusCode::NO_CONTENT, format!("sucess")));
}
