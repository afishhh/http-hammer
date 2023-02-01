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

pub mod generic_header_map {
    use hyper::{header::HeaderName, HeaderMap};
    use serde::{de::MapAccess, Deserialize, Deserializer};

    pub fn deserialize<'de, D, V: Deserialize<'de> + 'de>(de: D) -> Result<HeaderMap<V>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<'de, V: Deserialize<'de>>(std::marker::PhantomData<&'de V>);

        impl<'de, V: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<'de, V> {
            type Value = HeaderMap<V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid uri")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                struct WrappedName(
                    #[serde(with = "crate::config::serde_http::header_name")] HeaderName,
                );

                let mut headers = HeaderMap::<V>::default();

                while let Some((name, value)) = access.next_entry::<WrappedName, V>()? {
                    headers.insert(name.0, value);
                }

                Ok(headers)
            }
        }

        de.deserialize_map(Visitor(Default::default()))
    }
}
