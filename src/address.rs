use serde_with::DeserializeFromStr;
use std::{
    borrow::Borrow,
    convert::TryFrom,
    fmt::{Debug, Display, Formatter, Result as FmtResult, Write},
    net::{SocketAddr, ToSocketAddrs},
    ops::Deref,
    str::FromStr,
};

#[derive(Clone, Debug, DeserializeFromStr, PartialEq, Eq)]
pub struct Address(String);

impl Address {
    const DEF_HOST: &'static str = "127.0.0.1";
    const DEF_PORT: u16 = 80;

    pub fn new<S: Into<String>>(host: S, port: u16) -> Result<Self, Error> {
        let mut host = host.into();
        if let Err(source) = write!(host, ":{}", port) {
            return Err(Error::CreationFailed { host, port, source });
        }
        Ok(Self(host))
    }

    pub fn sock_addr_v4(&self) -> Result<SocketAddr, Error> {
        self.to_socket_addrs()
            .map_err(Error::ResolvingFailed)?
            .find(|x| matches!(x, SocketAddr::V4(_)))
            .ok_or_else(|| Error::NoIpv4Resolved(self.to_string()))
    }

    pub fn validate(text: &str) -> Result<(), Error> {
        let delimiter_position = text
            .find(':')
            .ok_or_else(|| Error::ParsingNoDelimiter(text.into()))?;
        let port = &text[(delimiter_position + 1)..];
        let _: u16 = port.parse().map_err(|source| Error::ParsingWrongPort {
            port: port.into(),
            source,
        })?;
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn inner(self) -> String {
        self.0
    }
}

impl ToSocketAddrs for Address {
    type Iter = std::vec::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.0.to_socket_addrs()
    }
}

impl Default for Address {
    fn default() -> Self {
        Self::new(Self::DEF_HOST, Self::DEF_PORT).expect("Failed creating default Address instance")
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::validate(text)?;
        Ok(Self(text.into()))
    }
}

impl TryFrom<String> for Address {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::validate(&value)?;
        Ok(Self(value))
    }
}

impl From<Address> for String {
    fn from(src: Address) -> Self {
        src.inner()
    }
}

impl Deref for Address {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for Address {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Address {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

pub type AddressList = Vec<Address>;

#[derive(Debug, thiserror::Error)] // NOTE: impossible to derive from Clone because std::io::Error doesn't implement it
pub enum Error {
    #[error("Failed parsing host and port: no delimiting ':' found in '{0}'")]
    ParsingNoDelimiter(String),
    #[error("Failed parsing port '{port}': {source}")]
    ParsingWrongPort {
        port: String,
        source: std::num::ParseIntError,
    },
    #[error("Failed resolving socket addresses: {0}")]
    ResolvingFailed(#[source] std::io::Error),
    #[error("Failed resolving into IPv4 host and port '{0}'")]
    NoIpv4Resolved(String),
    #[error("Failed creating Address instance from host '{host}' and port '{port}': {source}")]
    CreationFailed {
        host: String,
        port: u16,
        source: std::fmt::Error,
    },
}
