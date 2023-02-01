use assert_cmd::Command;
use httptest::{
    all_of, any_of,
    matchers::{contains, eq, json_decoded, request, any, not},
    responders, Expectation, ServerPool,
};
use serde_json::json;
use test_log::test;

const BIN: &str = env!("CARGO_PKG_NAME");
static SERVER_POOL: ServerPool = ServerPool::new(4);

fn run(expectations: impl IntoIterator<Item = Expectation>, config: impl FnOnce(String) -> String) {
    let server = SERVER_POOL.get_server();

    for exp in expectations {
        server.expect(exp)
    }

    // FIXME: Support testing on operating systems without /dev/stdin
    Command::cargo_bin(BIN)
        .unwrap()
        .arg("-t")
        .arg("6")
        .arg("/dev/stdin")
        .write_stdin(config(format!("http://{}", server.addr())))
        .assert()
        .success();
}

#[test]
fn test_simple_get() {
    run(
        [Expectation::matching(all_of![
            request::method_path("GET", "/hello"),
            request::body("Hello, world!")
        ])
        .times(1000)
        .respond_with(responders::status_code(200))],
        |server| {
            format!(
                r#"
                    [[hammer]]
                    method = "GET"
                    uri = "{server}/hello"
                    body = "Hello, world!"
                    count = 1000
                "#
            )
        },
    )
}

#[test]
fn test_headers() {
    run(
        [Expectation::matching(all_of![
            request::method_path("GET", "/hello"),
            request::headers(contains(("content-type", "text/plain"))),
            request::headers(contains(("x-hello", "Hi"))),
            request::body("Hello, world!")
        ])
        .times(1000)
        .respond_with(responders::status_code(200))],
        |server| {
            format!(
                r#"
                    [headers]
                    X-Hello = "Hi"
                    X-Second = "one"

                    [[hammer]]
                    method = "GET"
                    uri = "{server}/hello"
                    body = "Hello, world!"
                    count = 1000

                    [hammer.headers]
                    Content-Type = "text/plain"
                    X-Second = {{}}
                "#
            )
        },
    )
}

#[test]
fn test_cookies() {
    run(
        [Expectation::matching(all_of![
            request::method_path("GET", "/hello"),
            // TODO: Make a cookie matcher
            any_of![
                request::headers(contains((
                    "cookie",
                    "hello=wow%20very%20cool; another=cookie%21"
                ))),
                request::headers(contains((
                    "cookie",
                    "another=cookie%21; hello=wow%20very%20cool"
                )))
            ]
        ])
        .times(1000)
        .respond_with(responders::status_code(200))],
        |server| {
            format!(
                r#"
                    [cookies]
                    hello = "wow very cool"
                    deleted = "I will be deleted"

                    [[hammer]]
                    method = "GET"
                    uri = "{server}/hello"
                    count = 1000

                    [hammer.cookies]
                    another = "cookie!"
                    deleted = {{}}
                "#
            )
        },
    )
}

#[test]
fn test_resources() {
    const TOKEN: &str = "a-very-secret-value";

    run(
        [
            Expectation::matching(all_of![request::method_path("POST", "/login"),])
                .respond_with(responders::status_code(200).body(TOKEN)),
            Expectation::matching(all_of![
                request::method_path("GET", "/hello"),
                request::body(TOKEN)
            ])
            .times(1000)
            .respond_with(responders::status_code(200)),
        ],
        |server| {
            format!(
                r#"
                    [resources.token]
                    method = "POST"
                    uri = "{server}/login"

                    [[hammer]]
                    method = "GET"
                    uri = "{server}/hello"
                    body = "${{resources.token}}"
                    count = 1000
                "#
            )
        },
    )
}

#[test]
fn test_everything() {
    const AUTH_SCHEMA: &str = "Bearer";
    const COOKIE_NAME: &str = "token";
    const TOKEN: &str = "a-very-secret-value";

    run(
        [
            Expectation::matching(all_of![
                request::method_path("POST", "/login"),
                request::headers(contains(("content-type", "application/json"))),
                request::body(json_decoded(eq(
                    json!({ "username": "admin", "password": "admin" })
                )))
            ])
            .respond_with(responders::json_encoded(json!({ "token": TOKEN }))),
            Expectation::matching(all_of![
                request::method_path("GET", "/hello"),
                request::headers(contains(("content-type", "text/plain"))),
                request::headers(contains(("x-hello", "Hi"))),
                request::headers(contains((
                    "authorization",
                    format!("{AUTH_SCHEMA} {TOKEN}")
                ))),
                request::body("Hello, world!")
            ])
            .times(1000)
            .respond_with(responders::status_code(200)),
            Expectation::matching(all_of![
                request::method_path("PUT", "/hello2"),
                request::headers(contains(("content-type", "application/json"))),
                request::headers(contains(("cookie", format!("{COOKIE_NAME}={TOKEN}")))),
                request::headers(not(contains(("X-Hello", any())))),
            ])
            .times(1000)
            .respond_with(responders::status_code(200)),
        ],
        |server| {
            format!(
                r#"
                    [resources.token]
                    method = "POST"
                    uri = "{server}/login"
                    body = '''{{ "username": "admin", "password": "admin" }}'''
                    headers = {{ Content-Type = "application/json" }}
                    extract = {{ format = "json", pointer = "/token" }}
                    format = "{{}}"

                    [headers]
                    X-Hello = "Hi"
                    Content-Type = "application/json"

                    [[hammer]]
                    method = "GET"
                    uri = "{server}/hello"
                    body = "Hello, world!"
                    count = 1000

                    [hammer.headers]
                    Content-Type = "text/plain"
                    Authorization = "{AUTH_SCHEMA} ${{resources.token}}"

                    [[hammer]]
                    method = "PUT"
                    uri = "{server}/hello2"
                    count = 1000

                    [hammer.cookies]
                    {COOKIE_NAME} = "${{resources.token}}"
                    X-Hello = {{}}
                "#
            )
        },
    )
}
