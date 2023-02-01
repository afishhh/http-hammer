use std::{borrow::Cow, collections::HashMap, sync::Arc};

use anyhow::{anyhow, bail, Context, Result};
use async_recursion::async_recursion;
use hyper::client::connect::Connect;
use serde::Deserialize;
use tokio::sync::Mutex;

use super::{
    format::{format_callback, format_one},
    AlmostRequest, RequestInfo,
};

pub struct Evaluator<C: Connect + Clone + Send + Sync + 'static> {
    pub client: hyper::Client<C>,
    pub verbose: bool,
    pub resources: HashMap<String, Mutex<Value>>,
    pub request_cache: Mutex<HashMap<AlmostRequest, String>>,
}

#[derive(Debug, Clone, Deserialize, Hash, PartialEq, Eq)]
#[serde(tag = "format")]
enum BodyExtract {
    #[serde(rename = "json")]
    Json { pointer: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct FromResponseBody {
    #[serde(flatten)]
    request: RequestInfo,
    extract: Option<BodyExtract>,
    format: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Formatted(String),
    // FIXME: Implement deserialization for constant values
    Constant(String),
    Request(FromResponseBody),
}

impl From<String> for Value {
    fn from(val: String) -> Self {
        // This is just a heuristic, no need to be 100% accurate
        if val.contains('{') || val.contains('}') {
            Value::Formatted(val)
        } else {
            Value::Constant(val)
        }
    }
}

impl FromResponseBody {
    #[async_recursion]
    pub async fn resolve<C: Connect + Clone + Send + Sync + 'static>(
        self,
        evaluator: Arc<Evaluator<C>>,
    ) -> Result<String> {
        // FIXME: entry().or_insert_with_key(|| {}) cannot be used here because we need to use
        //        await in the insert callback
        let request = self.request.build(evaluator.clone()).await?;
        let mut cache = evaluator.request_cache.lock().await;
        let body: &str = match cache.get(&request) {
            Some(string) => string,
            None => {
                if evaluator.verbose {
                    eprintln!("Executing {} {}", request.method(), request.uri());
                }

                drop(cache);
                let response = evaluator.client.request(request.clone().into()).await?;

                cache = evaluator.request_cache.lock().await;
                // FIXME: This could be a try_insert instead.
                cache.insert(
                    request.clone(),
                    String::from_utf8(hyper::body::to_bytes(response.into_body()).await?.to_vec())?,
                );
                cache.get(&request).unwrap()
            }
        };

        let extracted = match self.extract {
            Some(BodyExtract::Json { pointer }) => {
                let value = serde_json::from_str::<serde_json::Value>(body)
                    .context("Failed to deserialize response")?;

                let val = value
                    .pointer(&pointer)
                    .context("Response does not contain expected value")?;

                Cow::Owned(if val.is_string() {
                    val.as_str().unwrap().to_string()
                } else {
                    val.to_string()
                })
            }
            None => Cow::Borrowed(body),
        };

        let formatted = {
            if let Some(fmtstr) = self.format {
                Cow::Owned(format_one(fmtstr, &extracted)?)
            } else {
                extracted
            }
        };

        Ok(formatted.to_string())
    }
}

impl Value {
    pub async fn evaluate<C>(self, evaluator: Arc<Evaluator<C>>) -> Result<String>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        Ok(match self {
            Self::Constant(cnst) => cnst,
            Self::Formatted(fmtstr) => format_with_resources(evaluator.clone(), &fmtstr).await?,
            Self::Request(req) => req.resolve(evaluator.clone()).await?,
        })
    }

    pub async fn evaluate_ref<C>(&mut self, evaluator: Arc<Evaluator<C>>) -> Result<String>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        Ok(match *self {
            Value::Constant(ref cnst) => cnst.clone(),
            Value::Formatted(ref fmtstr) => {
                let resolved = format_with_resources(evaluator, fmtstr.as_str()).await?;
                *self = Value::Constant(resolved.clone());
                resolved
            }
            ref mut value @ Value::Request(_) => {
                // FIXME: This enum dance theoretically could be avoided but
                //        requires uninitialized memory - aka. UB
                let req = match std::mem::replace(value, Value::Constant(String::new())) {
                    Value::Request(req) => req,
                    _ => unreachable!(),
                };
                let resolved = req.resolve(evaluator).await?;

                match *value {
                    Value::Constant(ref mut cnst) => {
                        cnst.clone_from(&resolved);
                    }
                    _ => unreachable!(),
                }

                resolved
            }
        })
    }

    pub async fn resolve_resource<C>(
        evaluator: Arc<Evaluator<C>>,
        resource: &str,
    ) -> Result<Option<String>>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        Ok(match evaluator.clone().resources.get(resource) {
            Some(rv) => {
                if let Ok(mut vlock) = rv.try_lock() {
                    Some(vlock.evaluate_ref(evaluator).await?)
                } else {
                    bail!("Cyclic dependency detected");
                }
            }
            None => None,
        })
    }
}

#[async_recursion]
async fn format_with_resources<C: Connect + Clone + Send + Sync + 'static>(
    evaluator: Arc<Evaluator<C>>,
    fmtstr: &str,
) -> Result<String> {
    format_callback(fmtstr, |fmtspec| {
        let evaluator = evaluator.clone();
        async move {
            let resource = fmtspec
                .strip_prefix("resources.")
                .ok_or_else(|| anyhow!("{fmtspec} must start with resources."))?;

            Value::resolve_resource(evaluator, resource)
                .await
                .and_then(|x| x.ok_or_else(|| anyhow!("Resource {resource} does not exist")))
        }
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MaybeDeleted<V = Value> {
    Deleted(Deleted),
    Value(V),
}

#[derive(Debug, Clone, Copy)]
pub struct Deleted;

impl<'de> Deserialize<'de> for Deleted {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Deleted;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "an empty map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                match map.size_hint() {
                    Some(0) => Ok(Deleted),
                    Some(_) => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Map,
                        &self,
                    )),
                    None => {
                        if map
                            .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                            .is_some()
                        {
                            Err(serde::de::Error::invalid_value(
                                serde::de::Unexpected::Map,
                                &self,
                            ))
                        } else {
                            Ok(Deleted)
                        }
                    }
                }
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}
