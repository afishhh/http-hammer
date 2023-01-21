use std::collections::HashMap;

use hyper::{header::HeaderName, http::HeaderValue, HeaderMap, Method, Uri};
use serde::Deserialize;

use crate::cookie::Cookie;

#[derive(Debug, Clone)]
pub struct HammerFile {
    pub hammer: Vec<HammerInfo>,
}

impl<'de> Deserialize<'de> for HammerFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct HammerFileRaw {
            #[serde(default)]
            cookies: HashMap<String, String>,
            hammer: Vec<HammerInfoRaw>,
        }

        let raw = HammerFileRaw::deserialize(deserializer)?;
        let base_cookie = Cookie::from(raw.cookies);

        let mut hammer = vec![];
        for raw_hammer in raw.hammer {
            hammer.push(HammerInfo::from_raw::<D>(raw_hammer, base_cookie.clone())?);
        }

        Ok(HammerFile { hammer })
    }
}

#[derive(Deserialize)]
struct HammerInfoRaw {
    name: Option<String>,
    uri: String,
    method: Option<String>,
    cookies: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    #[serde(default = "String::new")]
    body: String,
    count: u64,
    max_concurrency: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HammerInfo {
    pub name: String,
    pub uri: Uri,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: String,
    pub count: u64,
    pub max_concurrency: Option<u64>,
}

impl<'de> HammerInfo {
    // FIXME: This error handling is messy
    fn from_raw<D: serde::Deserializer<'de>>(
        raw: HammerInfoRaw,
        mut cookie: Cookie,
    ) -> std::result::Result<Self, D::Error> {
        struct Expected {
            text: &'static str,
        }
        impl Expected {
            fn new(text: &'static str) -> Self {
                Self { text }
            }
        }
        impl serde::de::Expected for Expected {
            fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "{}", self.text)
            }
        }

        // FIXME: The error messages here aren't very informative
        let parsed_uri: Uri = raw.uri.parse().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&raw.uri),
                &Expected::new("a valid url"),
            )
        })?;
        let parsed_method = match raw.method {
            Some(method) => method.parse().map_err(|_| {
                serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(&method),
                    &Expected::new("an http method"),
                )
            })?,
            None => Method::GET,
        };

        let mut new_headers = HeaderMap::new();

        if let Some(headers) = raw.headers {
            for (name, value) in headers.into_iter() {
                new_headers.insert(
                    HeaderName::try_from(&name).map_err(|_| {
                        serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(&name),
                            &Expected::new("a valid http header name"),
                        )
                    })?,
                    HeaderValue::try_from(&value).map_err(|_| {
                        serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(&value),
                            &Expected::new("a valid http header value"),
                        )
                    })?,
                );
            }
        }

        if let Some(cookies) = raw.cookies {
            cookie.extend(cookies.into_iter());
            new_headers.insert(hyper::header::COOKIE, cookie.to_header_value());
        }

        Ok(HammerInfo {
            name: raw
                .name
                .unwrap_or_else(|| format!("{parsed_method} {parsed_uri}")),
            uri: parsed_uri,
            method: parsed_method,
            headers: new_headers,
            body: raw.body,
            count: raw.count,
            max_concurrency: raw.max_concurrency,
        })
    }
}
