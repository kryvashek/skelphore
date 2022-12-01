pub mod address;
pub mod credentials;
#[cfg(feature = "pinger")]
pub mod ping;
pub mod timeoutsmap;

use const_format::formatcp;
use reqwest::{Client, Method, RequestBuilder, Url};
use serde::Deserialize;
use std::{
    convert::{TryFrom, TryInto},
    fmt::{Debug, Formatter, Result as FmtResult},
    hash::Hash,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use address::Address;
use credentials::Credentials;
#[cfg(feature = "pinger")]
use ping::{pinger, Behaviour, Handling, MinimalBehaviour, NoHandling};
use timeoutsmap::{Params, TimeoutsMap, TimeoutsMapConfig, TrivialKey, TrivialParams};

pub use reqwest;

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

#[derive(Debug, Deserialize, Clone)]
pub struct HostConfig<K: Eq + Hash + Into<usize> + Default> {
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

impl<K: Eq + Hash + Into<usize> + Default> Default for HostConfig<K> {
    fn default() -> Self {
        Self {
            credentials: None,
            target: Address::default(),
            scheme: Default::default(),
            timeouts: Default::default(),
            #[cfg(feature = "pinger")]
            ping: None,
        }
    }
}

#[cfg(feature = "pinger")]
#[derive(Debug)]
pub enum PingState<H> {
    Config(ping::Config),
    Handle(H),
}

#[cfg(feature = "pinger")]
struct HostInner<P: Params = TrivialParams, H: Handling = NoHandling> {
    client: Client,
    base_url: Url,
    timeouts: TimeoutsMap<P>,
    ping: Option<PingState<H::Handle>>,
}

#[cfg(not(feature = "pinger"))]
struct HostInner<P: Params = TrivialParams> {
    client: Client,
    base_url: Url,
    timeouts: TimeoutsMap<P>,
}

fn base_url(scheme: &'static str, instance: Address) -> Result<Url, Error> {
    let candidate = format!("{}://{}", scheme, instance);
    Url::from_str(&candidate).map_err(|source| Error::UrlParse { candidate, source })
}

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> HostInner<P, H> {
    pub fn new(config: HostConfig<P::Key>) -> Result<Self, Error> {
        let HostConfig {
            credentials,
            target,
            scheme,
            timeouts,
            ping,
        } = config;

        let mut client = Client::builder().user_agent(formatcp!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ));

        if let Some(cred_vals) = credentials {
            client =
                client.default_headers(cred_vals.try_into().map_err(Error::CredentialsConvert)?)
        }

        let client = client
            .https_only(matches!(scheme, Scheme::Https))
            .build()
            .map_err(Error::ClientBulid)?;

        let base_url = base_url(scheme.into(), target)?;

        Ok(Self {
            client,
            base_url,
            timeouts: TimeoutsMap::<P>::from(timeouts),
            ping: ping.map(PingState::Config),
        })
    }

    pub fn url(&self, path: &str) -> Url {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url
    }

    fn request_builder(&self, method: Method, path: &str, timeout: Duration) -> RequestBuilder {
        self.client.request(method, self.url(path)).timeout(timeout)
    }

    pub fn request(
        &self,
        method: Method,
        path: &str,
        spec: Option<P::Key>,
        xri: &str,
    ) -> RequestBuilder {
        let timeout = self.timeouts[spec.unwrap_or_default()];
        self.request_builder(method, path, timeout)
            .header("X-Request-Id", xri)
    }

    pub fn set_pinger<B: Behaviour<Handling = H>>(&mut self) -> bool {
        let ping_state = match self.ping.take() {
            None => return false,
            Some(config) => config,
        };
        let ping::Config {
            path,
            method,
            period,
        } = match ping_state {
            PingState::Handle(_) => return true,
            PingState::Config(config) => config,
        };
        let request = self.request_builder(method, &path, period);
        self.ping = Some(PingState::Handle(pinger::<B>(request, period)));
        true
    }
}

#[cfg(not(feature = "pinger"))]
impl<P: Params> HostInner<P> {
    pub fn new(config: HostConfig<P::Key>) -> Result<Self, Error> {
        let HostConfig {
            credentials,
            target,
            scheme,
            timeouts,
        } = config;

        let mut client = Client::builder().user_agent(formatcp!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ));

        if let Some(cred_vals) = credentials {
            client =
                client.default_headers(cred_vals.try_into().map_err(Error::CredentialsConvert)?)
        }

        let client = client
            .https_only(matches!(scheme, Scheme::Https))
            .build()
            .map_err(Error::ClientBulid)?;

        let base_url = base_url(scheme.into(), target)?;

        Ok(Self {
            client,
            base_url,
            timeouts: TimeoutsMap::<P>::from(timeouts),
        })
    }

    pub fn url(&self, path: &str) -> Url {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url
    }

    fn request_builder(&self, method: Method, path: &str, timeout: Duration) -> RequestBuilder {
        self.client.request(method, self.url(path)).timeout(timeout)
    }

    pub fn request(
        &self,
        method: Method,
        path: &str,
        spec: Option<P::Key>,
        xri: &str,
    ) -> RequestBuilder {
        let timeout = self.timeouts[spec.unwrap_or_default()];
        self.request_builder(method, path, timeout)
            .header("X-Request-Id", xri)
    }
}

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> Drop for HostInner<P, H> {
    fn drop(&mut self) {
        if let Some(PingState::Handle(handle)) = self.ping.take() {
            H::stop(handle)
        }
    }
}

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> TryFrom<HostConfig<P::Key>> for HostInner<P, H> {
    type Error = Error;

    fn try_from(value: HostConfig<P::Key>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(not(feature = "pinger"))]
impl<P: Params> TryFrom<HostConfig<P::Key>> for HostInner<P> {
    type Error = Error;

    fn try_from(value: HostConfig<P::Key>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(feature = "pinger")]
pub struct Host<P: Params = TrivialParams, H: Handling = NoHandling>(Arc<HostInner<P, H>>);

#[cfg(not(feature = "pinger"))]
pub struct Host<P: Params = TrivialParams>(Arc<HostInner<P>>);

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> Host<P, H> {
    pub fn new<B: Behaviour<Handling = H>>(config: HostConfig<P::Key>) -> Result<Self, Error> {
        let mut inner: HostInner<P, H> = config.try_into()?;
        inner.set_pinger::<B>();
        Ok(Self(Arc::new(inner)))
    }

    #[inline]
    pub fn post(&self, path: &str, spec: Option<P::Key>, xri: &str) -> RequestBuilder {
        self.0.request(Method::POST, path, spec, xri)
    }

    #[inline]
    pub fn get(&self, path: &str, spec: Option<P::Key>, xri: &str) -> RequestBuilder {
        self.0.request(Method::GET, path, spec, xri)
    }

    #[inline]
    pub fn request(
        &self,
        method: Method,
        path: &str,
        spec: Option<P::Key>,
        xri: &str,
    ) -> RequestBuilder {
        self.0.request(method, path, spec, xri)
    }
}

#[cfg(not(feature = "pinger"))]
impl<P: Params> Host<P> {
    pub fn new(config: HostConfig<P::Key>) -> Result<Self, Error> {
        Ok(Self(Arc::new(config.try_into()?)))
    }

    #[inline]
    pub fn post(&self, path: &str, spec: Option<P::Key>, xri: &str) -> RequestBuilder {
        self.0.request(Method::POST, path, spec, xri)
    }

    #[inline]
    pub fn get(&self, path: &str, spec: Option<P::Key>, xri: &str) -> RequestBuilder {
        self.0.request(Method::GET, path, spec, xri)
    }

    #[inline]
    pub fn request(
        &self,
        method: Method,
        path: &str,
        spec: Option<P::Key>,
        xri: &str,
    ) -> RequestBuilder {
        self.0.request(method, path, spec, xri)
    }
}

#[cfg(feature = "pinger")]
impl Default for Host<TrivialParams, NoHandling> {
    fn default() -> Self {
        Self::new::<MinimalBehaviour>(HostConfig::<TrivialKey>::default())
            .expect("Failed creating default Host instance")
    }
}

#[cfg(not(feature = "pinger"))]
impl Default for Host<TrivialParams> {
    fn default() -> Self {
        Self::new(HostConfig::<TrivialKey>::default())
            .expect("Failed creating default Host instance")
    }
}

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> Debug for Host<P, H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Host")
            .field("base_url", &self.0.base_url)
            .finish()
    }
}

#[cfg(not(feature = "pinger"))]
impl<P: Params> Debug for Host<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Host")
            .field("base_url", &self.0.base_url)
            .finish()
    }
}

#[cfg(feature = "pinger")]
impl<P: Params, H: Handling> Clone for Host<P, H> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[cfg(not(feature = "pinger"))]
impl<P: Params> Clone for Host<P> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[derive(Debug, thiserror::Error)] // NOTE: impossible to derive from Clone because reqwest::Error doesn't implement it
pub enum Error {
    #[error("Failed parsing URL from text '{candidate}': {source}")]
    UrlParse {
        candidate: String,
        source: <Url as FromStr>::Err,
    },
    #[error("Failed building HTTP(S) client: {0}")]
    ClientBulid(#[source] reqwest::Error),
    #[error(transparent)]
    CredentialsConvert(credentials::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::timeoutsmap::tests::{Spec, SpecParams};

    #[cfg(feature = "pinger")]
    #[test]
    fn config_read_and_apply() {
        let mut config: HostConfig<Spec> = toml::from_str(
            r#"
                name = "login"
                key = "pass"
                target = "example.com:4321"
                scheme = "http"
                timeouts = { default = "100ms", alice = "200ms" }
                ping = { period = "4s", path = "healthcheck", method = "GET" }
            "#,
        )
        .expect("Config should deserialize smoothly");

        assert_eq!(
            config.credentials,
            Some(Credentials {
                name: "login".into(),
                key: "pass".into(),
            })
        );
        assert_eq!(
            config.target,
            Address::new("example.com", 4321)
                .expect("Address should be created as 'example.com:4321'")
        );
        assert_eq!(config.scheme, Scheme::Http);
        assert_eq!(config.timeouts.default, Duration::from_millis(100));
        assert_eq!(config.timeouts.map.len(), 1);
        assert_eq!(
            config
                .timeouts
                .map
                .get(&Spec::Alice)
                .expect("Value for Spec::Alice should be presented")
                .into_inner(),
            Duration::from_millis(200)
        );

        let ping = config
            .ping
            .take()
            .expect("Pinger config should be presented");

        assert_eq!(ping.period, Duration::from_secs(4));
        assert_eq!(ping.path, "healthcheck");
        assert_eq!(ping.method, Method::GET);

        let _ = Host::<SpecParams, NoHandling>::new::<MinimalBehaviour>(config)
            .expect("Host instance should be created from config smoothly");
    }

    #[cfg(not(feature = "pinger"))]
    #[test]
    fn config_read_and_apply() {
        let config: HostConfig<Spec> = toml::from_str(
            r#"
                name = "login"
                key = "pass"
                target = "example.com:4321"
                scheme = "http"
                timeouts = { default = "100ms", alice = "200ms" }
            "#,
        )
        .expect("Config should deserialize smoothly");

        assert_eq!(
            config.credentials,
            Some(Credentials {
                name: "login".into(),
                key: "pass".into(),
            })
        );
        assert_eq!(
            config.target,
            Address::new("example.com", 4321)
                .expect("Address should be created as 'example.com:4321'")
        );
        assert_eq!(config.scheme, Scheme::Http);
        assert_eq!(config.timeouts.default, Duration::from_millis(100));
        assert_eq!(config.timeouts.map.len(), 1);
        assert_eq!(
            config
                .timeouts
                .map
                .get(&Spec::Alice)
                .expect("Value for Spec::Alice should be presented")
                .into_inner(),
            Duration::from_millis(200)
        );

        let _ = Host::<SpecParams>::new(config)
            .expect("Host instance should be created from config smoothly");
    }
}
