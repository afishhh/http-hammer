use std::{collections::HashMap, fmt::Display};

use hyper::http::HeaderValue;

#[derive(Debug, Clone)]
pub struct Cookie {
    cookies: HashMap<String, String>,
}

impl Cookie {
    pub fn extend(&mut self, iterator: impl Iterator<Item = (String, String)>) {
        self.cookies.extend(iterator);
    }

    pub fn to_header_value(&self) -> HeaderValue {
        self.to_string().try_into().unwrap()
    }
}

impl From<HashMap<String, String>> for Cookie {
    fn from(map: HashMap<String, String>) -> Self {
        Self { cookies: map }
    }
}

impl Display for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut not_first = false;

        for (name, value) in self.cookies.iter() {
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
