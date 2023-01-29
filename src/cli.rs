use std::path::PathBuf;

use clap::ValueHint;

#[derive(clap::Parser)]
#[command(about, version)]
pub struct Args {
    /// Specify how many tasks to use for hammering.
    #[arg(long, short, default_value_t = 1, value_parser = clap::value_parser!(u64).range(0..))]
    pub tasks: u64,

    /// TOML file with hammering configuration.
    ///
    /// # Format
    /// It should contain an array of tables called "hammer" where each table should have the
    /// following properties:
    ///     'uri': a string containing a valid uri
    ///     'count' a number specifying how many reqeusts to make
    ///
    /// It can also have these optional properties:
    ///     'method': a string containing the HTTP method to use
    ///     'cookies': a cookie name -> cookie value map
    ///                a cookie value may also be '{}' which unsets that cookie if it was
    ///                previously set in the global cookies table
    ///     'headers': a header name -> header value map
    ///     'body': a string used as the body for the request
    ///     'name': a string displayed while hammering instead of the default `${METHOD} ${URI}` name
    ///     'max_concurrency': a number representing the maximum number of tasks that should be used
    ///                        to hammer the url
    ///
    /// Also optionally, a 'cookies' table may be specified at the top level which will be
    /// propagated to all other entries in the file.
    ///
    /// # Example entry
    /// [[hammer]]
    /// name = "my endpoint"
    /// uri = "http://127.0.0.1:8000/do_something"
    /// method = "POST"
    /// cookie = { "some-cookie" = "value" }
    /// headers = { "Content-Type" = "application/json" }
    /// body = '''
    ///   { "do":"thing" }
    /// '''
    /// count = 20000
    /// max_concurrency = 10
    #[arg(verbatim_doc_comment, value_hint = ValueHint::FilePath)]
    pub config: PathBuf,
}
