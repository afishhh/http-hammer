use std::future::Future;

use anyhow::{bail, Result};

pub fn format_one(mut fmtstr: String, value: &str) -> Result<String> {
    let mut format_spec_index = None;

    {
        let mut prev_was_lbrace = false;
        for (i, chr) in fmtstr.chars().enumerate() {
            if chr == '{' {
                prev_was_lbrace = !prev_was_lbrace;
            } else {
                if chr == '}' && prev_was_lbrace {
                    if format_spec_index.is_some() {
                        bail!("There has to be exactly one format specifier in a format string");
                    }
                    format_spec_index = Some(i)
                }

                prev_was_lbrace = false;
            }
        }
    }

    match format_spec_index {
        Some(idx) => {
            fmtstr.replace_range((idx - 1)..=idx, value);
            Ok(fmtstr)
        }
        None => {
            bail!("There has to be exactly one format specifier in a format string")
        }
    }
}

pub async fn format_callback<FF: Future<Output = Result<String>>, F: FnMut(String) -> FF>(
    fmtstr: &str,
    mut callback: F,
) -> Result<String> {
    let mut out = String::with_capacity(fmtstr.len());

    enum State {
        Normal,
        Spec { value: String },
    }

    let mut state = State::Normal;
    let mut it = fmtstr.chars().peekable();
    loop {
        match (&mut state, it.next()) {
            (State::Normal, Some('$')) => match it.peek() {
                Some('$') => out.push('$'),
                Some('{') => {
                    #[cfg(debug_assertions)]
                    assert_eq!(it.next(), Some('{'));
                    #[cfg(not(debug_assertions))]
                    it.next();

                    state = State::Spec {
                        value: String::new(),
                    };
                }
                Some(c) => {
                    bail!("Unexpected '{c}' encountered after '$', expected either '{{' or '$'")
                }
                None => bail!("Unexpected EOF encountered after '$'"),
            },
            (State::Normal, Some(c)) => out.push(c),
            (State::Normal, None) => break,
            (State::Spec { .. }, Some(c @ ('$' | '{'))) => {
                bail!("Format specifiers cannot contain '{c}'")
            }
            (State::Spec { .. }, Some('}')) => {
                let value = match std::mem::replace(&mut state, State::Normal) {
                    State::Normal => unreachable!(),
                    State::Spec { value } => value,
                };

                out.push_str(&callback(value).await?)
            }
            (State::Spec { value }, Some(c)) => value.push(c),
            (State::Spec { .. }, None) => {
                bail!("Unexpected EOF encountered while parsing format specifier")
            }
        }
    }

    Ok(out)
}
