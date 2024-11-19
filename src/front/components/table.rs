use std::ops::Sub;

use anyhow::anyhow;
use bigdecimal::num_traits::CheckedMul;
use serde::{Deserialize, Serialize};

pub type Url = String;

const DEFAULT_ENTRIES_PER_PAGE: u32 = 25;
const MAX_ENTRIES_PER_PAGE: u32 = 1000;

#[derive(Serialize, Default)]
pub struct Page {
    page_number: u32,
    is_current_page: bool,
    link: Url,
}

#[derive(Serialize, Default)]
pub struct TableComponent<T: Serialize + Default> {
    entries: Vec<T>,
    pages: Vec<Page>,
    first_page: Option<Url>,
    last_page: Option<Page>,
    previous_page: Option<Url>,
    next_page: Option<Url>,
    columns: Vec<String>,
    max_entries_per_page: u32,
}

#[derive(Deserialize)]
pub struct Query {
    page: Option<u32>,
    entries_per_page: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct QueryNormalized {
    page: u32,
    entries_per_page: u32,
}

impl QueryNormalized {
    pub fn limit(&self) -> i64 {
        self.entries_per_page as i64
    }
    pub fn offset(&self) -> i64 {
        (self
            .page
            .sub(1)
            .checked_mul(self.entries_per_page)
            .unwrap_or(0)) as i64
    }
}

impl Query {
    pub fn noramlize(&self) -> QueryNormalized {
        QueryNormalized {
            page: self.page.unwrap_or(1).min(u32::MAX).max(1),
            entries_per_page: self
                .entries_per_page
                .unwrap_or(DEFAULT_ENTRIES_PER_PAGE)
                .max(1)
                .min(MAX_ENTRIES_PER_PAGE),
        }
    }
}

impl<T: Serialize + Default> TableComponent<T> {
    pub fn new(
        entries: Vec<T>,
        count: i64,
        api_path: impl AsRef<str>,
        query: QueryNormalized,
    ) -> anyhow::Result<Self> {
        let number_of_pages = (count as f64 / query.entries_per_page as f64).ceil() as u32;
        let current_page = query.page;
        let mut component = Self::default();
        component.entries = entries;
        component.columns = get_struct_fields_names(T::default())?;
        component.max_entries_per_page = query.entries_per_page;
        component.pages = (current_page as i32 - 3..=current_page as i32 + 3)
            .filter(|p| *p >= 1)
            .filter(|p| *p <= number_of_pages as i32)
            .map(|p| Page {
                page_number: p as u32,
                is_current_page: p == current_page as i32,
                link: format!("{}?page={}", api_path.as_ref(), p),
            })
            .collect();

        if component.pages.len() == 0 {
            return Ok(component);
        }

        if component.pages.last().unwrap().page_number != number_of_pages {
            component.last_page = Some(Page {
                page_number: number_of_pages,
                is_current_page: false,
                link: format!("{}?page={}", api_path.as_ref(), number_of_pages),
            })
        };

        if component.pages.first().unwrap().page_number != 1 {
            component.first_page = Some(format!("{}?page=1", api_path.as_ref()))
        };

        if current_page != number_of_pages {
            component.next_page = Some(format!("{}?page={}", api_path.as_ref(), current_page + 1))
        };

        if current_page != 1 {
            component.previous_page =
                Some(format!("{}?page={}", api_path.as_ref(), current_page - 1))
        };

        Ok(component)
    }
}

fn get_struct_fields_names(s: impl Serialize) -> anyhow::Result<Vec<String>> {
    let j = serde_json::to_value(s)?;
    let j = j.as_object().ok_or(anyhow!("it should be an object"))?;
    Ok(j.iter().map(|f| f.0).cloned().collect())
}
