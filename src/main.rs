use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use hyper::{header::HeaderName, http::HeaderValue, Body, Client, HeaderMap, Method, Request, Uri};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct HammerFile {
    hammer: Vec<HammerInfo>,
}

#[derive(Debug, Clone)]
struct HammerInfo {
    name: String,
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    body: String,
    count: u64,
}

impl<'de> Deserialize<'de> for HammerInfo {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct HammerInfoRaw {
            name: Option<String>,
            uri: String,
            method: Option<String>,
            headers: Option<HashMap<String, String>>,
            #[serde(default = "String::new")]
            body: String,
            count: u64,
        }

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

        let raw = HammerInfoRaw::deserialize(deserializer)?;

        // FIXME: The error messages here aren't very informative
        let parsed_uri: Uri = raw.uri.parse().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&raw.uri),
                &Expected::new("a valid url"),
            )
        })?;
        Ok(HammerInfo {
            name: raw.name.unwrap_or_else(|| parsed_uri.to_string()),
            uri: parsed_uri,
            method: match raw.method {
                Some(method) => method.parse().map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(&method),
                        &Expected::new("an http method"),
                    )
                })?,
                None => Method::GET,
            },
            headers: match raw.headers {
                Some(headers) => {
                    let mut new = HeaderMap::new();

                    for (name, value) in headers.into_iter() {
                        new.insert(
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

                    new
                }
                None => HeaderMap::new(),
            },
            body: raw.body,
            count: raw.count,
        })
    }
}

#[derive(clap::Parser)]
struct Args {
    /// Specify how many tasks to use for hammering.
    #[arg(long, short, default_value_t = 1, value_parser = clap::value_parser!(u64).range(0..))]
    tasks: u64,

    /// File with url and request count pairs.
    ///
    /// The file should contains newline delimited pairs
    /// of url and request count separated by a comma.
    /// ex.
    /// url, requests
    /// url2, requests2
    urls: PathBuf,
}

async fn real_main() -> Result<()> {
    let args = Args::parse();

    let mut buf = vec![];
    {
        let mut file = File::open(&args.urls).context("Could not open urls file")?;
        file.read_to_end(&mut buf)
            .context("Could not read urls file")?;
    }

    let urls = toml::de::from_slice::<HammerFile>(&buf).context("Could not parse urls file")?.hammer;

    // let client: Client<_, hyper::Body> =
    //     hyper::Client::builder().build(hyper_tls::HttpsConnector::new());
    let clients: Vec<Client<_, hyper::Body>> =
        std::iter::repeat_with(|| hyper::Client::builder().build(hyper_tls::HttpsConnector::new()))
            .take(args.tasks as usize)
            .collect();

    for info in urls {
        let todo = Arc::new(AtomicU64::from(info.count));
        let error_encountered = Arc::new(AtomicBool::new(false));

        let mut handles = vec![];

        for tidx in 0..args.tasks {
            let info = info.clone();
            let client = clients[tidx as usize].clone();
            let todo = todo.clone();
            let error_encountered = error_encountered.clone();
            let error_encountered2 = error_encountered.clone();

            handles.push(tokio::spawn(async move {
                let result = (|| async move {
                    let mut max = std::time::Duration::ZERO;
                    let mut min = std::time::Duration::MAX;
                    let mut sum = std::time::Duration::ZERO;
                    let mut done: u64 = 0;

                    while todo
                        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| x.checked_sub(1))
                        .is_ok()
                        && !error_encountered2.load(Ordering::Relaxed)
                    {
                        let request = {
                            let mut request = Request::builder()
                                .method(info.method.clone())
                                .uri(info.uri.clone());

                            *request.headers_mut().unwrap() = info.headers.clone();

                            request.body(Body::from(info.body.clone()))?
                        };

                        let start = std::time::Instant::now();

                        // let response = client.get(url.clone()).await?;
                        let response = client.request(request).await?;
                        if !response.status().is_success() {
                            bail!(
                                "GET {} returned non-200 status code {}",
                                info.uri,
                                response.status()
                            );
                        }
                        hyper::body::to_bytes(response.into_body()).await?;

                        let end = std::time::Instant::now();
                        let dur = end - start;

                        max = std::cmp::max(max, dur);
                        min = std::cmp::min(min, dur);
                        sum += dur;
                        done += 1;
                    }

                    Ok((min, sum, done, max)) as anyhow::Result<_>
                })()
                .await;

                if result.is_err() {
                    error_encountered.store(true, Ordering::Release);
                }

                result
            }));
        }

        let mut previous = VecDeque::new();
        loop {
            let now = std::time::Instant::now();
            let todo = todo.load(Ordering::Relaxed);
            let done = info.count - todo;

            if todo == 0 || error_encountered.load(Ordering::Relaxed) {
                break;
            }

            let per_sec = (if previous.len() > 5 {
                previous.pop_front()
            } else {
                previous.front().copied()
            })
            .map(|(prev, prev_done)| {
                let dur: std::time::Duration = now - prev;
                let change = done - prev_done;
                (change as f64) / dur.as_secs_f64()
            });
            previous.push_back((now, done));

            eprint!(
                "\x1b[2KHammering {} \x1b[33;1m{done}/{}\x1b[0m",
                info.name, info.count
            );
            if let Some(per_sec) = per_sec {
                eprint!(" (\x1b[94;1m{per_sec:.0}/s\x1b[0m)");
            }
            eprint!("\r");
            std::io::stderr()
                .flush()
                .context("Could not flush stderr")?;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        if !error_encountered.load(Ordering::Acquire) {
            eprintln!(
                "\x1b[2KHammering {} \x1b[32;1m{count}/{count}\x1b[0m",
                info.name,
                count = info.count
            );
        } else {
            eprintln!()
        }

        let mut max = std::time::Duration::ZERO;
        let mut min = std::time::Duration::MAX;
        let mut sum = std::time::Duration::ZERO;
        let mut done = 0;

        for handle in handles {
            let (tmin, tsum, tdone, tmax) = handle.await??;

            max = std::cmp::max(max, tmax);
            min = std::cmp::min(min, tmin);
            sum += tsum;
            done += tdone;
        }

        assert_eq!(done, info.count);

        println!("Results for {}:", info.uri);
        println!(
            "    min {:.2}ms avg {:.2}ms max {:.2}ms",
            min.as_secs_f64() * 1000.0,
            (sum.as_secs_f64() / done as f64) * 1000.0,
            max.as_secs_f64() * 1000.0,
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    match real_main().await {
        Ok(value) => value,
        Err(error) => {
            let mut chain = error.chain().enumerate();
            eprintln!(
                "\x1b[31;1mRuntime error\x1b[0m: {}",
                chain.next().unwrap().1
            );
            for (i, error) in chain {
                eprintln!("\x1b[31;1m#{i}\x1b[0m: {error}");
            }
        }
    }
}
