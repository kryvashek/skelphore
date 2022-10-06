use cubob::{Alternate, StructShow};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::{
    convert::TryFrom,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
};

#[derive(Debug, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub key: String,
}

impl TryFrom<Credentials> for HeaderMap<HeaderValue> {
    type Error = Error;

    fn try_from(src: Credentials) -> Result<Self, Self::Error> {
        let Credentials { name, key } = src;
        let mut header_map = HeaderMap::with_capacity(2);
        header_map.insert_from_string("X-API-Name", name)?;
        header_map.insert_from_string("X-API-Key", key)?;
        Ok(header_map)
    }
}

impl Display for Credentials {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        StructShow::new(f, Alternate::OneLine)
            .field(&"name", &self.name)
            .field(&"key", &self.key)
            .finish()
    }
}

trait HeaderMapInsertString {
    type Fail: std::error::Error;

    fn insert_from_string(&mut self, key: &'static str, value: String) -> Result<(), Self::Fail>;
}

impl HeaderMapInsertString for HeaderMap<HeaderValue> {
    type Fail = Error;

    fn insert_from_string(&mut self, key: &'static str, val: String) -> Result<(), Self::Fail> {
        let val = HeaderValue::from_str(&val).map_err(|source| Error::InvalidHeaderValue {
            source,
            key,
            val,
        })?;
        self.insert(key, val);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)] // NOTE: impossible to derive from Clone because reqwest::header::InvalidHeaderValue doesn't implement it
pub enum Error {
    #[error("Failed making header value for header '{key}' from text '{val}': {source}")]
    InvalidHeaderValue {
        source: reqwest::header::InvalidHeaderValue,
        key: &'static str,
        val: String,
    },
}
