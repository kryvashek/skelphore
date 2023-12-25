use std::hash::Hash;

use serde::Deserialize;

use crate::{address::Address, credentials::Credentials, timeoutsmap::TimeoutsMapConfig, Scheme};

#[cfg(feature = "pinger")]
use crate::ping;

#[derive(Debug, Deserialize, Clone)]
pub struct HostConfig<K: Eq + Hash + Default> {
    #[serde(default, flatten)]
    pub credentials: Option<Credentials>,
    #[serde(default)]
    pub target: Address,
    #[serde(default)]
    pub scheme: Scheme,
    #[serde(default)]
    pub timeouts: TimeoutsMapConfig<K>,
    #[cfg(feature = "pinger")]
    #[serde(default)]
    pub ping: Option<ping::Config>,
}

impl<K: Eq + Hash + Default> Default for HostConfig<K> {
    fn default() -> Self {
        Self {
            credentials: None,
            target: Default::default(),
            scheme: Default::default(),
            timeouts: Default::default(),
            #[cfg(feature = "pinger")]
            ping: None,
        }
    }
}
