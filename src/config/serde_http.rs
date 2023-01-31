pub mod method {
    use hyper::Method;
    use serde::{
        de::{Error, Unexpected},
        Deserializer,
    };

    pub fn deserialize<'de, D>(de: D) -> Result<Method, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Method;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid method name")
            }

            fn visit_str<E: Error>(self, val: &str) -> Result<Self::Value, E> {
                val.parse()
                    .map_err(|_| Error::invalid_value(Unexpected::Str(val), &self))
            }
        }

        de.deserialize_str(Visitor)
    }
}

pub mod uri {
    use hyper::Uri;
    use serde::{
        de::{Error, Unexpected},
        Deserializer,
    };

    pub fn deserialize<'de, D>(de: D) -> Result<Uri, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Uri;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid uri")
            }

            fn visit_str<E: Error>(self, val: &str) -> Result<Self::Value, E> {
                val.parse()
                    .map_err(|_| Error::invalid_value(Unexpected::Str(val), &self))
            }
        }

        de.deserialize_str(Visitor)
    }
}

pub mod header_name {
    use hyper::header::HeaderName;
    use serde::{
        de::{Error, Unexpected},
        Deserializer,
    };

    pub fn deserialize<'de, D>(de: D) -> Result<HeaderName, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HeaderName;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid header name")
            }

            fn visit_str<E: Error>(self, val: &str) -> Result<Self::Value, E> {
                val.parse()
                    .map_err(|_| Error::invalid_value(Unexpected::Str(val), &self))
            }
        }

        de.deserialize_str(Visitor)
    }
}

pub mod header_value {
    use hyper::header::HeaderValue;
    use serde::{
        de::{Error, Expected, SeqAccess, Unexpected},
        Deserializer,
    };

    pub fn deserialize<'de, D>(de: D) -> Result<HeaderValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HeaderValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid header value")
            }

            fn visit_str<E: Error>(self, val: &str) -> Result<Self::Value, E> {
                val.parse()
                    .map_err(|_| Error::invalid_value(Unexpected::Str(val), &self))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                struct ByteExpected;
                impl Expected for ByteExpected {
                    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "a valid header value byte")
                    }
                }

                let mut out = vec![];

                while let Some(value) = access.next_element::<u8>()? {
                    // HeaderValue::from_bytes documentation says that only 32-255 \ 127 bytes
                    // are permitted in header values.
                    if (32..=255).contains(&value) && value != 127 {
                        out.push(value);
                    } else {
                        return Err(Error::invalid_value(
                            Unexpected::Unsigned(value.into()),
                            &ByteExpected,
                        ));
                    }
                }

                Ok(HeaderValue::try_from(out).unwrap())
            }
        }

        de.deserialize_any(Visitor)
    }
}

pub mod header_map {
    use hyper::{header::HeaderName, http::HeaderValue, HeaderMap};
    use serde::{de::MapAccess, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(de: D) -> Result<HeaderMap, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HeaderMap;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid uri")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                struct WrappedName(
                    #[serde(with = "crate::config::serde_http::header_name")] HeaderName,
                );

                #[derive(Deserialize)]
                struct WrappedValue(
                    #[serde(with = "crate::config::serde_http::header_value")] HeaderValue,
                );

                let mut headers = HeaderMap::new();

                while let Some((name, value)) = access.next_entry::<WrappedName, WrappedValue>()? {
                    headers.insert(name.0, value.0);
                }

                Ok(headers)
            }
        }

        de.deserialize_map(Visitor)
    }
}
