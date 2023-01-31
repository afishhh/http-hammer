<p align="center" width="100%">
	<img src="assets/banner.svg"/>
</p>
<h1 align="center">HTTP Hammer</h1>

A tool for testing HTTP(S) endpoint throughput and response time.

### Installation

<!-- TODO: Add Windows installation instructions -->

#### Linux
Currently there are no official packages available but the project can be built and installed manually using `cargo`.
First make sure you have the appropriate rust toolchain installed (on *most* Linux distributions this should be done through [rustup](https://rustup.rs)), then execute the following command:
```
cargo install --git=https://github.com/afishhh/http-hammer
```

When you want to update `http-hammer` rerun that command again.

> **Note**
> By default `http-hammer` will use your system installation of libssl for HTTPS.
> - To use [rustls](https://github.com/rustls/rustls) as the SSL implementation add these flags to the end of the `cargo install` command:
> `--no-default-features --features=rustls`
> - To build without HTTPS support add this flag to the end of the `cargo install` command: `--no-default-features`

Afterwards you can try running `http-hammer` in your terminal, if the shell cannot find the binary make sure that `$HOME/.cargo/bin` is in your `$PATH` and try again.

#### NixOS with flakes
If you're using NixOS with flakes you can add this repository as an input, and then apply its default overlay.
This will add an `http-hammer` package to your pkgs which you can install like any other nix package.

> **Note**
> This will use openssl as the SSL backend, there is no way to change this (yet).

### Usage

> **Warning**
> This tool was created for testing HTTP endpoints controlled by you, using this on other websites without permission from the owner is forbidden and most likely illegal depending on the laws in your country!

----

First a configuration file has to be created, the syntax and available options are described in the [Configuration](#configuration) section.

The tool can then be run like this: `http-hammer -t <CONCURRENT_TASKS> <PATH_TO_CONFIG>` where
- `<CONCURRENT_TASKS>` is the max number of concurrent tasks `http-hammer` will use to make connections.
	> **Note**
	> This can be limited (but not increased) on a per-endpoint basis in the configuration file.
- `<PATH_TO_CONFIG>` is the path to your newly created configuration file.

If the `-t` flag is omitted a default value of `1` will be used.

### Configuration
`http-hammer` expects the [TOML](https://toml.io) configuration file to contain a list of tables called `hammer` and an optional table `cookies`.

The `hammer` tables specify the different API endpoints to test and can have the following properties.
- `uri` the URI of the http endpoint.
- `count` how many requests to send.
- (optional) `method` a HTTP method for the hammer requests, default: `GET`.
- (optional) `cookie` a table of cookie name and value pairs, cookies names and values will both be URL encoded, a cookie can be set to an empty table (`{}`) to remove it (if it was set by the global `cookies` table then it will be overridden).
- (optional) `headers` a table of header name and value pairs, headers names and values will NOT be URL encoded and thus must be valid HTTP header names and values..
- (optional) `body` an HTTP body of for the hammer requests, default: empty.
- (optional) `name` a human readable name that will be displayed while testing, default: `$method $uri`.
- (optional) `max_concurrency` a limit for the amount of tasks to use for hammering. `http-hammer` will use `min($max_concurrency, $cli_concurrency)` where `cli_concurrency` is the number passed to the binary via the `-t` flag.

The `cookie` table specifies global cookies that will be inherited by all hammer entries in the file, as with the `cookie` property of `hammer`, these will be URL encoded but setting a cookie to `{}` is disallowed since it makes no sense here.

##### Examples
- Send 1000 GET requests to `http://127.0.0.1:8000`:
```toml
[[hammer]]
name = "homepage"
uri = "http://127.0.0.1:8000/"
count = 1000
```

- Send 1000 POST requests to `https://127.0.0.1:8000/login` with a custom body:
```toml
[[hammer]]
name = "login"
uri = "https://127.0.0.1:8000/login"
method = "POST"
count = 1000
body = '''{
	"username": "admin",
	"password": "hunter2"
}'''
```

- Send 1000 POST requests to `https://127.0.0.1:8000/add` with custom body, cookie and headers:
```toml
[[hammer]]
name = "login"
uri = "https://127.0.0.1:8000/add"
method = "POST"
count = 1000
headers = { "Authorization" = "Bearer test-token" }
cookies = { "user_id" = "abcdefgh" }
body = '''{
	"title": "Hello, world!",
	"content": "This is content."
}'''
```

- Send 100 GET requests to `https://127.0.0.1:8000/view` with the cookies `shared-cookie-one`, `shared-cookie-two` and `cookie-three` then another 100 GET requests to `https://127.0.0.1:8000/view` but with only `shared-cookie-one`.
```toml
[cookies]
shared-cookie-one = "one"
shared-cookie-two = "two"

[[hammer]]
uri = "https://127.0.0.1:8000/view"
count = 100
cookies = { "cookie-three" = "three" }

[[hammer]]
uri = "https://127.0.0.1:8000/view"
count = 100
cookies = { "shared-cookie-two" = {} }
```

### License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
