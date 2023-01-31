use std::{
    collections::HashMap,
    fmt::Display,
    ops::{Deref, DerefMut},
};

use hyper::http::HeaderValue;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum CookieValue {
    Deleted,
    Set(String),
}

impl CookieValue {
    pub fn into_option(self) -> Option<String> {
        match self {
            CookieValue::Deleted => None,
            CookieValue::Set(value) => Some(value),
        }
    }

    pub fn as_option(&self) -> Option<&str> {
        match self {
            CookieValue::Deleted => None,
            CookieValue::Set(value) => Some(value),
        }
    }
}

impl<'de> Deserialize<'de> for CookieValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            MapNull(HashMap<(), ()>),
            String(String),
        }

        // FIXME: This error message is terrible
        Ok(match Raw::deserialize(deserializer)? {
            Raw::MapNull(_) => Self::Deleted,
            Raw::String(value) => Self::Set(value),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Cookie {
    cookies: HashMap<String, CookieValue>,
}

impl Cookie {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_header_value(&self) -> HeaderValue {
        self.to_string().try_into().unwrap()
    }

    pub fn iter_set(&self) -> impl Iterator<Item = (&str, &str)> {
        self.cookies
            .iter()
            .filter_map(|(a, b)| b.as_option().map(|value| (a.as_str(), value)))
    }
}

impl Deref for Cookie {
    type Target = HashMap<String, CookieValue>;

    fn deref(&self) -> &Self::Target {
        &self.cookies
    }
}

impl DerefMut for Cookie {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cookies
    }
}

impl Display for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut not_first = false;

        for (name, value) in self.iter_set() {
            if not_first {
                write!(f, "; ")?;
            }
            not_first = true;

            write!(
                f,
                "{}={}",
                urlencoding::encode(name).as_ref(),
                urlencoding::encode(value).as_ref()
            )?;
        }

        Ok(())
    }
}

impl<'de> Deserialize<'de> for Cookie {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            cookies: HashMap::<String, CookieValue>::deserialize(deserializer)?,
        })
    }
}
