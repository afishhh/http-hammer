use std::collections::HashMap;

use hyper::{HeaderMap, Method, Request, Uri};
use serde::Deserialize;

use crate::{
    cookie::{Cookie, CookieValue},
    USER_AGENT,
};

mod serde_http;

#[derive(Debug, Clone)]
pub struct HammerFile {
    pub hammer: Vec<HammerInfo>,
}

impl HammerFile {
    pub fn parse_toml(text: &str) -> Result<HammerFile, toml::de::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            cookies: HashMap<String, String>,
            hammer: Vec<HammerInfo>,
        }

        let raw: Raw = toml::from_str(text)?;
        let mut hammers = raw.hammer;

        for hammer in hammers.iter_mut() {
            let other_cookie = &mut hammer.request.cookie;

            for (key, value) in raw.cookies.iter() {
                other_cookie
                    .entry(key.to_string())
                    .or_insert_with(|| CookieValue::Set(value.to_string()));
            }
        }

        Ok(HammerFile { hammer: hammers })
    }
}

fn method_get() -> Method {
    Method::GET
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestInfo {
    #[serde(with = "serde_http::uri")]
    pub uri: Uri,
    #[serde(with = "serde_http::method", default = "method_get")]
    pub method: Method,
    #[serde(rename = "cookies", default = "Cookie::new")]
    pub cookie: Cookie,
    #[serde(with = "serde_http::header_map", default = "HeaderMap::new")]
    pub headers: HeaderMap,
    #[serde(default = "String::new")]
    pub body: String,
}

impl From<RequestInfo> for Request<hyper::Body> {
    fn from(val: RequestInfo) -> Self {
        let mut request = Request::builder().method(val.method).uri(val.uri);

        *request.headers_mut().unwrap() = val.headers;

        request
            // FIXME: Theoretically the cookie conversion is unnecessarily executed many times here
            .header(hyper::header::COOKIE, val.cookie.as_header_value())
            .header(
                hyper::header::USER_AGENT,
                hyper::http::HeaderValue::from_static(USER_AGENT),
            )
            .body(hyper::Body::from(val.body))
            .unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct HammerInfo {
    pub name: String,
    pub request: RequestInfo,
    pub count: u64,
    pub max_concurrency: Option<u64>,
}

impl<'de> Deserialize<'de> for HammerInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            name: Option<String>,
            #[serde(flatten)]
            request: RequestInfo,
            count: u64,
            max_concurrency: Option<u64>,
        }

        let raw = Raw::deserialize(deserializer)?;

        Ok(HammerInfo {
            name: raw
                .name
                .unwrap_or_else(|| format!("{} {}", raw.request.method, raw.request.uri)),
            request: raw.request,
            count: raw.count,
            max_concurrency: raw.max_concurrency,
        })
    }
}
