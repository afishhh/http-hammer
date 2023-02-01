use std::{collections::HashMap, hash::Hash, sync::Arc};

use anyhow::{Context, Result};
use async_recursion::async_recursion;
use hyper::{
    client::connect::Connect, header::COOKIE, http::HeaderValue, HeaderMap, Method, Request, Uri,
};
use serde::Deserialize;

use crate::{config::eval::Value, cookie::Cookie, USER_AGENT};

pub mod eval;
pub mod format;
pub mod serde_http;
use eval::{Evaluator, MaybeDeleted};

#[derive(Debug, Clone)]
pub struct HammerFile {
    pub resources: HashMap<String, Value>,
    pub hammer: Vec<HammerInfo>,
}

impl HammerFile {
    pub fn parse_toml(text: &str) -> Result<HammerFile, toml::de::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            cookies: HashMap<String, String>,
            #[serde(with = "serde_http::generic_header_map", default)]
            headers: HeaderMap<String>,
            #[serde(default)]
            resources: HashMap<String, Value>,
            hammer: Vec<HammerInfo>,
        }

        let raw: Raw = toml::from_str(text)?;
        let mut hammers = raw.hammer;

        for hammer in hammers.iter_mut() {
            for (key, value) in raw.cookies.iter() {
                hammer
                    .request
                    .cookies
                    .entry(key.to_string())
                    .or_insert_with(|| MaybeDeleted::Value(value.to_string().into()));
            }

            for (key, value) in raw.headers.iter() {
                hammer
                    .request
                    .headers
                    .entry(key)
                    .or_insert_with(|| MaybeDeleted::Value(value.to_string().into()));
            }
        }

        Ok(HammerFile {
            resources: raw.resources,
            hammer: hammers,
        })
    }
}

fn method_get() -> Method {
    Method::GET
}

fn boxed_empty_value() -> Box<Value> {
    Box::new(Value::empty())
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestInfo {
    #[serde(with = "serde_http::uri")]
    pub uri: Uri,
    #[serde(with = "serde_http::method", default = "method_get")]
    pub method: Method,
    #[serde(default = "HashMap::new")]
    pub cookies: HashMap<String, MaybeDeleted>,
    #[serde(with = "serde_http::generic_header_map", default)]
    pub headers: HeaderMap<MaybeDeleted>,
    // This has to be boxed since a Value may eventually contain another Value
    #[serde(default = "boxed_empty_value")]
    pub body: Box<Value>,
}

#[derive(Clone, PartialEq, Eq)]
/// A type that is not an [`http::Request`](hyper::http::Request) but can be cheaply converted to
/// one while also implementing [`Clone`].
pub struct AlmostRequest {
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    body: String,
}

impl RequestInfo {
    #[async_recursion]
    pub async fn build<C: Connect + Clone + Send + Sync + 'static>(
        self,
        evaluator: Arc<Evaluator<C>>,
    ) -> Result<AlmostRequest> {
        let mut headers = HeaderMap::new();

        if evaluator.verbose > 0 {
            eprintln!("Building request {} {}", self.method, self.uri);
        }

        {
            let mut cookie = Cookie::new();

            for (name, value) in self.cookies {
                cookie.add(
                    &name,
                    &match value {
                        MaybeDeleted::Deleted(_) => continue,
                        MaybeDeleted::Value(value) => {
                            if evaluator.verbose > 0 {
                                eprintln!("Resolving value for cookie {name}");
                            }

                            value.evaluate(evaluator.clone()).await.with_context(|| {
                                format!("Failed to resolve value for cookie {name}")
                            })?
                        }
                    },
                )
            }

            headers.insert(COOKIE, cookie.into());
        }

        {
            for (name, value) in self
                .headers
                .into_iter()
                .filter_map(|(no, v)| no.map(|n| (n, v)))
            {
                let val = match value {
                    MaybeDeleted::Deleted(_) => continue,
                    MaybeDeleted::Value(value) => {
                        if evaluator.verbose > 0 {
                            eprintln!("Resolving value for header {name}");
                        }

                        value
                            .evaluate(evaluator.clone())
                            .await
                            .with_context(|| format!("Failed to resolve value for header {name}"))?
                    }
                };
                let hval = HeaderValue::try_from(val).with_context(|| {
                    format!("Value for header {name} is not a valid header value")
                })?;

                headers.insert(name, hval);
            }
        }

        Ok(AlmostRequest {
            uri: self.uri,
            method: self.method,
            headers,
            body: self
                .body
                .evaluate(evaluator)
                .await
                .context("Failed to resolve value for body")?,
        })
    }
}

// FIXME: This is not really a FIXME since this issue is very hard so solve differently.
//        Implementing Hash for a HashMap is non-trivial but since this function is called
//        infrequently so a naive slow solution was chosen.
#[allow(clippy::derive_hash_xor_eq)]
impl Hash for AlmostRequest {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
        self.method.hash(state);

        {
            let mut vec = self.headers.iter().collect::<Vec<_>>();
            vec.sort_unstable_by(|a, b| {
                use std::cmp::Ordering;

                match a.0.as_str().cmp(b.0.as_str()) {
                    ord @ (Ordering::Greater | Ordering::Less) => ord,
                    Ordering::Equal => a.1.as_bytes().cmp(b.1.as_bytes()),
                }
            });
            vec.hash(state);
        }

        self.body.hash(state);
    }
}

impl From<AlmostRequest> for Request<hyper::Body> {
    fn from(val: AlmostRequest) -> Self {
        let mut request = Request::builder().method(val.method).uri(val.uri);

        *request.headers_mut().unwrap() = val.headers;

        request
            .header(
                hyper::header::USER_AGENT,
                hyper::http::HeaderValue::from_static(USER_AGENT),
            )
            .body(hyper::Body::from(val.body))
            .unwrap()
    }
}

impl AlmostRequest {
    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
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
