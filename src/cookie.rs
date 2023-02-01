use std::fmt::Write;

use hyper::http;

#[derive(Debug, Clone, Default)]
pub struct Cookie(String);

impl Cookie {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, value: &str) {
        if !self.0.is_empty() {
            self.0.push_str("; ");
        }

        write!(
            self.0,
            "{}={}",
            urlencoding::encode(name).as_ref(),
            urlencoding::encode(value).as_ref()
        )
        .unwrap();
    }
}

impl Into<http::HeaderValue> for Cookie {
    fn into(self) -> http::HeaderValue {
        self.0.try_into().unwrap()
    }
}

impl<'a, A: Into<&'a str>> Extend<(A, A)> for Cookie {
    fn extend<T: IntoIterator<Item = (A, A)>>(&mut self, iter: T) {
        for (n, v) in iter.into_iter() {
            self.add(n.into(), v.into());
        }
    }
}

impl<'a, A: Into<&'a str>> FromIterator<(A, A)> for Cookie {
    fn from_iter<T: IntoIterator<Item = (A, A)>>(iter: T) -> Self {
        let mut n = Self::new();
        n.extend(iter);
        n
    }
}
