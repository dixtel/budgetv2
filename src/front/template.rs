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

#[derive(Clone)]
pub struct Template {
    r: Arc<RwLock<handlebars::Handlebars<'static>>>,
}

impl Template {
    pub fn new() -> Self {
        let mut handlebars = handlebars::Handlebars::new();
        for entitiy in fs::read_dir("./src/front/templates").unwrap() {
            let entity = entitiy.unwrap();
            handlebars
                .register_template_file(entity.file_name().to_str().unwrap(), entity.path())
                .unwrap();
        }

        handlebars_helper!(nor_amt: |i: String| format!("{:.02}", i.parse::<f64>().unwrap_or(0.0)));

        handlebars.register_helper("nor_amt", Box::new(nor_amt));
        Self {
            r: Arc::new(RwLock::new(handlebars)),
        }
    }

    pub fn render<T>(&self, name: &str, data: &T) -> Response
    where
        T: Serialize,
    {
        println!("render '{}': {:#?}", name, serde_json::to_value(data));
        match self.r.read().unwrap().render(name, data) {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}
