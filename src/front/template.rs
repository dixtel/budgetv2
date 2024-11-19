use std::{
    fs,
    sync::{Arc, RwLock},
};

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use handlebars::{handlebars_helper, RenderError};
use serde::Serialize;
use serde_json::Value;

/// Condig standards:
/// - helpers functions should be in snake_case
/// - template variables should be in snake_case
#[derive(Clone)]
pub struct Template {
    r: Arc<RwLock<handlebars::Handlebars<'static>>>,
}

impl Template {
    pub fn new() -> Self {
        let mut handlebars = handlebars::Handlebars::new();
        for entitiy in fs::read_dir("./src/front/templates").unwrap() {
            let entity = entitiy.unwrap();
            let res = handlebars
                .register_template_file(entity.file_name().to_str().unwrap(), entity.path());
            if let Err(err) = res {
                log::info!("{}", err);
                panic!("stop");
            }
        }

        handlebars_helper!(normalizeAmount: |i: String| format!("{:.02}", i.parse::<f64>().unwrap_or(0.0)));
        handlebars_helper!(range: |a1: Value, a2: Value | (a1.as_i64().unwrap()..=a2.as_i64().unwrap()).collect::<Vec<i64>>() );
        handlebars_helper!(toMonthString: |m: Value| {
            match m.as_i64().unwrap_or(0) {
                1 => format!("January"),
                2 => format!("February"),
                3 => format!("March"),
                4 => format!("April"),
                5 => format!("May"),
                6 => format!("June"),
                7 => format!("July"),
                8 => format!("August"),
                9 => format!("September"),
                10 => format!("October"),
                11 => format!("November"),
                12 => format!("December"),
                _ => format!("?")
            }
        } );

        handlebars.register_helper("normalizeAmount", Box::new(normalizeAmount));
        handlebars.register_helper("toMonthString", Box::new(toMonthString));
        handlebars.register_helper("range", Box::new(range));
        Self {
            r: Arc::new(RwLock::new(handlebars)),
        }
    }

    pub fn render<T>(&self, name: &str, data: &T) -> anyhow::Result<Response>
    where
        T: Serialize,
    {
        log::info!("render '{}': {:#?}", name, serde_json::to_value(data));
        let html = Html(self.r.read().unwrap().render(name, data)?);
        Ok(html.into_response())
    }
}
