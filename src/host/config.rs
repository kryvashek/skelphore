use std::{hash::Hash, time::Duration};

use reqwest::ClientBuilder;
use serde::Deserialize;

use crate::{address::Address, credentials::Credentials, timeoutsmap::TimeoutsMapConfig, Scheme};

#[cfg(feature = "pinger")]
use crate::ping;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct HostConfig<K: Eq + Hash + Default> {
    /// Credentials to use for authentication (only X-API headers are currently supported).
    #[serde(default, flatten)]
    pub credentials: Option<Credentials>,
    /// Terget host address (IP or DNS-name and port separated with semicolon).
    #[serde(default)]
    pub target: Address,
    /// Scheme used to interact with the host (all requests will use that scheme).
    #[serde(default)]
    pub scheme: Scheme,
    #[serde(default)]
    /// Timeouts map for different request types (depends on K type parameter).
    pub timeouts: TimeoutsMapConfig<K>,
    #[cfg(feature = "pinger")]
    /// Autometed pinger configuration.
    #[serde(default)]
    pub ping: Option<ping::Config>,
    /// Extra settings to pass into related reqwest's ClientBuilder methods. If None, default reqwest's parameters are being kept.
    /// If not None, but empty (i.e. empty section in the config) provides its own defaults!
    #[serde(default)]
    pub extras: Option<ExtraSettings>,
}

/// Different parameters, being passed right into related reqwest's ClientBuilder methods.
#[derive(Debug, Deserialize, Clone)]
pub struct ExtraSettings {
    /// A timeout for only the connect phase of a Client. This requires the futures be executed in a tokio runtime with a tokio timer enabled!
    /// Default is None, which means no timeout.
    #[serde(default)]
    pub connect_timeout: Option<Duration>,
    /// Turns on/off verbouse connection logs (emitted with TRACE level for read and write operations on connections).
    /// Default is false.
    #[serde(default = "ExtraSettings::def_connection_verbose")]
    pub connection_verbose: bool,
    /// Optional timeout for idle sockets being kept-alive.
    /// Default is None, which means no timeout.
    #[serde(default)]
    pub pool_idle_timeout: Option<Duration>,
    /// The maximum idle connections count allowed in the pool.
    /// Default is usize::MAX.
    #[serde(default = "ExtraSettings::def_pool_max_idle_per_host")]
    pub pool_max_idle_per_host: usize,
    /// Turns on/off SO_KEEPALIVE with the supplied duration for used TCP sockets.
    /// Default is None, which means no KA.
    #[serde(default)]
    pub tcp_keepalive: Option<Duration>,
    /// Turns on/off TCP_NODELAY for used TCP sockets.
    /// Default is true.
    #[serde(default = "ExtraSettings::def_tcp_nodelay")]
    pub tcp_nodelay: bool,
}

impl ExtraSettings {
    fn def_connection_verbose() -> bool {
        false
    }

    fn def_pool_max_idle_per_host() -> usize {
        usize::MAX
    }

    fn def_tcp_nodelay() -> bool {
        true
    }

    pub fn apply(self, mut builder: ClientBuilder) -> ClientBuilder {
        if let Some(timeout) = self.connect_timeout {
            builder = builder.connect_timeout(timeout);
        }

        builder
            .connection_verbose(self.connection_verbose)
            .pool_idle_timeout(self.pool_idle_timeout)
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .tcp_keepalive(self.tcp_keepalive)
            .tcp_nodelay(self.tcp_nodelay)
    }
}

impl Default for ExtraSettings {
    fn default() -> Self {
        Self {
            connect_timeout: Default::default(),
            connection_verbose: Self::def_connection_verbose(),
            pool_idle_timeout: Default::default(),
            pool_max_idle_per_host: Self::def_pool_max_idle_per_host(),
            tcp_keepalive: Default::default(),
            tcp_nodelay: Self::def_tcp_nodelay(),
        }
    }
}
