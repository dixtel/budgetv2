pub mod api;

use std::any::Any;

use super::{
    components::{self, table::TableComponent},
    AppState,
};
use anyhow::anyhow;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Router,
};
use serde::Serialize;
use uuid::Uuid;

pub struct AppMessage(Response);

impl AppMessage {
    pub fn new_info_notification(msg: impl AsRef<str>, s: &AppState) -> AppMessage {
        #[derive(Serialize, Default)]
        struct Ctx {
            info: Option<String>,
            warn: Option<String>,
            error: Option<String>,
        }

        let mut ctx = Ctx::default();
        ctx.info = Some(msg.as_ref().to_string());

        Self(s.t.render("base.notification.hbs", &ctx).unwrap())
    }

    pub fn new_error_notification(msg: anyhow::Error, s: &AppState) -> AppMessage {
        #[derive(Serialize, Default)]
        struct Ctx {
            info: Option<String>,
            warn: Option<String>,
            error: Option<String>,
        }

        let mut ctx = Ctx::default();
        ctx.error = Some(msg.to_string());

        Self(s.t.render("base.notification.hbs", &ctx).unwrap())
    }

    pub fn new_error(msg: anyhow::Error, s: &AppState) -> AppMessage {
        #[derive(Serialize, Default)]
        struct Ctx {
            info: Option<String>,
            warn: Option<String>,
            error: Option<String>,
        }

        let mut ctx = Ctx::default();
        ctx.error = Some(msg.to_string());

        Self(s.t.render("error.get.hbs", &ctx).unwrap())
    }
}

impl IntoResponse for AppMessage {
    fn into_response(self) -> Response {
        self.0
    }
}

pub fn new_router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(get))
        .route("/", axum::routing::post(post))
}

#[axum::debug_handler]
async fn get(State(s): State<AppState>) -> Result<Response, AppMessage> {
    let res =
        s.t.render("accounts.get.hbs", &())
            .map_err(|err| AppMessage::new_error(anyhow!(err), &s))?;
    Ok(res)
}

#[axum::debug_handler]
async fn post(State(s): State<AppState>) -> Result<Response, AppMessage> {
    todo!()
}
