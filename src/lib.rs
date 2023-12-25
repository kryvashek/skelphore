pub mod address;
pub mod credentials;
pub mod host;
#[cfg(feature = "pinger")]
pub mod ping;
pub mod timeoutsmap;

use std::fmt::{Display, Formatter, Result as FmtResult};

pub use reqwest;
use serde::Deserialize;

pub use self::host::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Http,
    Https,
}

impl Default for Scheme {
    fn default() -> Self {
        Self::Https
    }
}

impl From<Scheme> for &str {
    fn from(src: Scheme) -> Self {
        match src {
            Scheme::Http => "http",
            Scheme::Https => "https",
        }
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str((*self).into())
    }
}
