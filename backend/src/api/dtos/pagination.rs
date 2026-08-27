use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct PaginationRequest {
    #[validate(range(min = 1))]
    #[serde(
        default = "default_page",
        deserialize_with = "deserialize_string_or_number"
    )]
    pub page: u64,

    #[validate(range(min = 1, max = 100))]
    #[serde(
        default = "default_page_size",
        deserialize_with = "deserialize_string_or_number"
    )]
    pub page_size: u64,

    #[serde(default)]
    pub search_text: Option<String>,
}

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(u64),
    }

    match StringOrInt::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse().map_err(serde::de::Error::custom),
        StringOrInt::Int(i) => Ok(i),
    }
}

pub fn deserialize_option_vec_or_single<'de, D, T>(
    deserializer: D,
) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum VecOrSingle<T> {
        Vec(Vec<T>),
        Single(T),
    }

    match Option::<VecOrSingle<T>>::deserialize(deserializer)? {
        Some(VecOrSingle::Vec(v)) => Ok(Some(v)),
        Some(VecOrSingle::Single(s)) => Ok(Some(vec![s])),
        None => Ok(None),
    }
}

fn default_page() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
}

#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: u64, page: u64, page_size: u64) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            (total + page_size - 1) / page_size
        };

        Self {
            data,
            meta: PaginationMeta {
                total,
                page,
                page_size,
                total_pages,
            },
        }
    }
}
