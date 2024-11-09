pub mod template;

use std::{
    collections::HashMap,
    error::Error,
    io::{self, Read},
};

use axum::{
    extract::{connect_info::MockConnectInfo, Multipart, Query, State},
    http::{status, StatusCode},
    response::{Html, IntoResponse, Redirect, Response, Result},
    routing::{get, post},
    Router,
};
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{Datelike, DurationRound, TimeDelta, TimeZone};
use log::Record;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::*, types, Pool, Postgres};
use tower_http::services::ServeDir;

use crate::models;

#[derive(Clone)]
struct AppState {
    p: Pool<Postgres>,
    t: template::Template,
}

pub async fn start_web_server(p: &Pool<Postgres>) {
    println!("loading templates...");
    let t = template::Template::new();

    // build our application with a single route
    let app = Router::new()
        .route("/", get(index))
        .route("/details", get(details))
        .route("/expenses", get(expenses))
        .route("/api/entry", get(api_entry))
        .route("/api/expenses", get(api_expenses))
        .route("/api/upload", post(api_upload))
        .nest_service("/public", ServeDir::new("./src/front/public"))
        .with_state(AppState { p: p.clone(), t });

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("open website at http://localhost:3000");
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

    s.t.render("entry_details.hbs", &Ctx { entry })
}

async fn expenses(State(s): State<AppState>) -> Response {
    #[derive(Serialize)]
    struct Ctx {}
    s.t.render("expenses.hbs", &Ctx {})
}

#[derive(Deserialize)]
struct ExpensesQuery {
    max_elements: Option<usize>,
    month: Option<String>,
    year: Option<String>,
}

async fn api_expenses(State(s): State<AppState>, Query(q): Query<ExpensesQuery>) -> Response {
    let year = chrono::Local::now().year();
    let mut year2 = year;
    let month = chrono::Local::now().month() - 1;
    let mut month2 = month;
    month2 += 1;
    if month2 == 13 {
        month2 = 1;
        year2 += 1;
    }

    const MAIN_ACCOUNT_ID: &'static str = "1e7a4379-4fd5-45df-ba1b-fd6f3fc34717";

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
        FROM 
            entry
        WHERE 
            source_account IN (
                SELECT reference FROM account_reference WHERE account_id::text = $1
            ) AND
            accounting_date >= $2::date AND
            accounting_date < $3::date AND
            amount < 0
        GROUP BY 
            category
        ORDER BY
            amount ASC
        "#,
    )
    .bind(MAIN_ACCOUNT_ID)
    .bind(format!("{}-{}-1", year, month))
    .bind(format!("{}-{}-1", year2, month2))
    .fetch_all(&s.p)
    .await
    .unwrap();

    if let Some(m) = q.max_elements {
        if records.len() > m {
            records.truncate(m);
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
        },
    )
}

#[derive(Deserialize)]
struct EntryQuery {
    page: Option<usize>,
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

    let max_page = (count as f64 / 10.0).ceil() as usize;

    if current_page == 0 {
        current_page = 1;
    } else if current_page >= max_page && max_page > 0 {
        current_page = max_page
    }

    let entries: Vec<models::Entry> =
        sqlx::query_as::<_, models::Entry>("SELECT * FROM entry LIMIT 10 OFFSET $1")
            .bind(((current_page - 1) * 10) as i64)
            .fetch_all(&s.p)
            .await
            .unwrap();

    #[derive(Serialize, Default)]
    struct Pagination {
        page: usize,
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
            page: p as usize,
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

    s.t.render("entry.hbs", &ctx)
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

async fn load(p: &Pool<Postgres>, s: impl io::Read) -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(s);

    let mut count = 0;

    for result in rdr.records() {
        let record = result?;

        let mapping: &[(usize, &str, Box<dyn Fn(String) -> String + Send>)] = &[
            (
                0,
                "accounting_date",
                Box::new(|v| format!("TO_DATE('{}', 'DD.MM.YYYY')", v)),
            ),
            (
                1,
                "currency_date",
                Box::new(|v| format!("TO_DATE('{}', 'DD.MM.YYYY')", v)),
            ),
            (2, "sender_or_receiver", Box::new(|v| format!("'{}'", v))),
            (3, "address", Box::new(|v| format!("'{}'", v))),
            (
                4,
                "source_account",
                Box::new(|v| format!("'{}'", v.replace("'", "''"))),
            ),
            (
                5,
                "destination_account",
                Box::new(|v| format!("'{}'", v.replace("'", "''"))),
            ),
            (6, "title", Box::new(|v| format!("'{}'", v))),
            (
                7,
                "amount",
                Box::new(|v| format!("{}", v.replace(",", ".").replace(" ", ""))),
            ),
            (8, "currency", Box::new(|v| format!("'{}'", v))),
            (
                9,
                "reference_number",
                Box::new(|v| format!("'{}'", v.replace("'", "''"))),
            ),
            (10, "operation_type", Box::new(|v| format!("'{}'", v))),
            (11, "category", Box::new(|v| format!("'{}'", v))),
        ];

        if record.len() < 11 {
            log::warn!("invalid record");
            continue;
        }

        let mut values: HashMap<&str, String> = HashMap::new();

        for (idx, field, map) in mapping {
            let value = &record[*idx];
            values.insert(field, map(value.to_string()));
        }

        let fields_query: Vec<&str> = mapping.iter().map(|v| v.1).collect();
        let fields_values: Vec<String> = fields_query
            .iter()
            .map(|v| values.get(v).unwrap().clone())
            .collect();
        let insert_query = format!(
            "INSERT INTO entry ({}) VALUES ({});",
            fields_query.join(", "),
            fields_values.join(", ")
        );

        match sqlx::query(&insert_query).execute(p).await {
            Ok(_) => {
                count += 1;
                ()
            }
            Err(err) => {
                if let Some(err) = err.as_database_error() {
                    if err.is_unique_violation() {
                        log::warn!("cannnt load entry: unique violation for entry {:?}", values);
                        continue;
                    }
                }
                println!("{}", insert_query);
                return Err(Box::new(err));
            }
        };
    }

    println!("{} records were loaded to db", count);

    Ok(())
}

#[axum_macros::debug_handler]
async fn api_upload(
    State(s): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, (StatusCode, String)> {
    while let Ok(Some(m)) = multipart.next_field().await {
        let file_name = m.file_name().unwrap();
        println!("file_name={}", file_name);

        let name = m.name().unwrap();
        println!("name={}", name);

        let bytes = m.bytes().await.unwrap();

        match load(&s.p, bytes.as_ref()).await {
            Ok(_) => (),
            Err(err) => log::error!("cannot load file: {}", err),
        }
    }

    return Err((StatusCode::NO_CONTENT, format!("sucess")));
}
