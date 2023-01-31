use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Write},
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use config::HammerFile;
use hyper::{client::connect::Connect, Client};

mod cli;
mod config;
mod cookie;
use cli::Args;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

struct TimeStats {
    pub max: std::time::Duration,
    pub min: std::time::Duration,
    pub sum: std::time::Duration,
    pub done: u64,
}

impl TimeStats {
    fn add(&mut self, dur: std::time::Duration) {
        self.max = std::cmp::max(self.max, dur);
        self.min = std::cmp::min(self.min, dur);
        self.sum += dur;
        self.done += 1;
    }

    fn min_secs(&self) -> f64 {
        self.min.as_secs_f64()
    }

    fn avg_secs(&self) -> f64 {
        self.sum.as_secs_f64() / self.done as f64
    }

    fn max_secs(&self) -> f64 {
        self.max.as_secs_f64()
    }

    fn append(&mut self, rhs: Self) {
        self.max = std::cmp::max(self.max, rhs.max);
        self.min = std::cmp::min(self.min, rhs.min);
        self.sum += rhs.sum;
        self.done += rhs.done;
    }
}

impl Default for TimeStats {
    fn default() -> Self {
        Self {
            max: std::time::Duration::ZERO,
            min: std::time::Duration::MAX,
            sum: std::time::Duration::ZERO,
            done: 0,
        }
    }
}

#[derive(Default)]
struct HammerStats {
    // For the (request sent)-(response received) time period
    pub response: TimeStats,
    // For the (request sent)-(body received) time period
    pub total: TimeStats,
}

impl HammerStats {
    fn append(&mut self, other: Self) {
        self.response.append(other.response);
        self.total.append(other.total);
    }
}

fn hyper_connector() -> impl Connect + Clone {
    #[cfg(feature = "nativels")]
    return hyper_tls::HttpsConnector::new();

    #[cfg(feature = "rustls")]
    return hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    #[cfg(all(not(feature = "rustls"), not(feature = "nativels")))]
    return hyper::client::HttpConnector::new();
}

async fn real_main() -> Result<ExitCode> {
    let args = Args::parse();

    let mut buf = String::new();
    {
        let mut file = File::open(&args.config).context("Could not open urls file")?;
        file.read_to_string(&mut buf)
            .context("Could not read urls file")?;
    }

    let urls = HammerFile::parse_toml(&buf)
        .context("Could not parse urls file")?
        .hammer;

    let client: Client<_, hyper::Body> = hyper::Client::builder().build(hyper_connector());

    for info in urls {
        let todo = Arc::new(AtomicU64::from(info.count));
        let error_encountered = Arc::new(AtomicBool::new(false));

        let mut handles = vec![];

        let tasks = info
            .max_concurrency
            .map(|x| x.min(args.tasks))
            .unwrap_or(args.tasks);
        for _ in 0..tasks {
            let info = info.clone();
            let client = client.clone();
            let todo = todo.clone();
            let error_encountered = error_encountered.clone();
            let error_encountered2 = error_encountered.clone();

            handles.push(tokio::spawn(async move {
                let result = (|| async move {
                    let mut stats = HammerStats::default();

                    while todo
                        .fetch_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
                        .is_ok()
                        && !error_encountered2.load(Ordering::Relaxed)
                    {
                        let request = info.request.clone().into();

                        let start = std::time::Instant::now();

                        let response = client.request(request).await?;

                        let responded = std::time::Instant::now();

                        if !response.status().is_success() {
                            bail!(
                                "{} {} returned non-200 status code {}",
                                info.request.method,
                                info.request.uri,
                                response.status()
                            );
                        }

                        hyper::body::to_bytes(response.into_body()).await?;

                        let end = std::time::Instant::now();

                        stats.response.add(responded - start);
                        stats.total.add(end - start);
                    }

                    Ok(stats) as anyhow::Result<HammerStats>
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
                "\x1b[2KHammering {} \x1b[33;1m{done}/{}\x1b[0m (\x1b[35;1m{tasks}\x1b[0m tasks",
                info.name, info.count
            );
            if let Some(per_sec) = per_sec {
                eprint!(", \x1b[94;1m{per_sec:.0}/s\x1b[0m");
            }
            eprint!(")\r");
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
            let done = info.count - todo.load(Ordering::Acquire);
            eprintln!(
                "\x1b[2KHammering {} \x1b[31;1mfailed\x1b[0m \x1b[33;1m{done}/{count}\x1b[0m",
                info.name,
                count = info.count
            );
        }

        let mut stats = HammerStats::default();
        let mut any_failed = false;
        for (tidx, handle) in handles.into_iter().enumerate() {
            match handle.await? {
                Ok(htr) => stats.append(htr),
                Err(e) => {
                    eprintln!("    Task {} \x1b[31;1mfailed\x1b[0m: {e}", tidx + 1);
                    any_failed = true;
                }
            }
        }

        if any_failed {
            return Ok(ExitCode::FAILURE);
        }

        assert_eq!(stats.total.done, info.count);

        println!(
            "    Initial response: min {:.2}ms avg {:.2}ms max {:.2}ms",
            stats.response.min_secs() * 1000.0,
            stats.response.avg_secs() * 1000.0,
            stats.response.max_secs() * 1000.0,
        );

        println!(
            "    Whole body: min {:.2}ms avg {:.2}ms max {:.2}ms",
            stats.total.min_secs() * 1000.0,
            stats.total.avg_secs() * 1000.0,
            stats.total.max_secs() * 1000.0,
        );
    }

    Ok(ExitCode::SUCCESS)
}

#[tokio::main]
async fn main() -> ExitCode {
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
            ExitCode::FAILURE
        }
    }
}
