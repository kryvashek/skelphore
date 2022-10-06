pub mod credentials;
pub mod hostandport;
pub mod ping;
pub mod timeoutsmap;

use const_format::formatcp;
use reqwest::{Client, Method, RequestBuilder, Url};
use serde::Deserialize;
use std::{
    convert::{TryFrom, TryInto},
    fmt::{Debug, Formatter, Result as FmtResult},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use credentials::Credentials;
use hostandport::HostAndPort;
use ping::{pinger, Behaviour, Handle, MinimalBehaviour, NoSpawnHandle, Spawn};
use timeoutsmap::{Key, Params, TimeoutsMap, TimeoutsMapConfig, TrivialKey, TrivialParams};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
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
pub struct HttpHostConfig<K: Key> {
    #[serde(default, flatten)]
    pub credentials: Option<Credentials>,
    #[serde(default)]
    pub target: HostAndPort,
    #[serde(default)]
    pub scheme: Scheme,
    #[serde(default)]
    pub timeouts: TimeoutsMapConfig<K>,
    #[serde(default)]
    pub ping: Option<ping::Config>,
}

impl<T: Key> Default for HttpHostConfig<T> {
    fn default() -> Self {
        Self {
            credentials: None,
            target: HostAndPort::default(),
            scheme: Default::default(),
            timeouts: Default::default(),
            ping: None,
        }
    }
}

#[derive(Debug)]
pub enum PingState<H> {
    Config(ping::Config),
    Handle(H),
}

struct HttpHostInner<P: Params = TrivialParams, H: Handle = NoSpawnHandle> {
    client: Client,
    base_url: Url,
    timeouts: TimeoutsMap<P>,
    ping: Option<PingState<H>>,
}

fn base_url(scheme: &'static str, instance: HostAndPort) -> Result<Url, Error> {
    let candidate = format!("{}://{}", scheme, instance);
    Url::from_str(&candidate).map_err(|source| Error::UrlParse { candidate, source })
}

impl<P: Params, H: Handle> HttpHostInner<P, H> {
    pub fn new(config: HttpHostConfig<P::Key>) -> Result<Self, Error> {
        let HttpHostConfig {
            credentials,
            target,
            scheme,
            timeouts,
            ping,
        } = config;

        let mut client =
            Client::builder().user_agent(formatcp!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));

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

    pub fn set_pinger<B>(&mut self) -> bool
    where
        B: Behaviour,
        B::Spawn: Spawn<Handle = H>,
    {
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

impl<P: Params, H: Handle> Drop for HttpHostInner<P, H> {
    fn drop(&mut self) {
        if let Some(PingState::Handle(handle)) = self.ping.take() {
            handle.stop()
        }
    }
}

impl<P: Params, H: Handle> TryFrom<HttpHostConfig<P::Key>> for HttpHostInner<P, H> {
    type Error = Error;

    fn try_from(value: HttpHostConfig<P::Key>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

pub struct HttpHost<P: Params = TrivialParams, H: Handle = NoSpawnHandle>(Arc<HttpHostInner<P, H>>);

impl<P: Params, H: Handle> HttpHost<P, H> {
    pub fn new<B>(config: HttpHostConfig<P::Key>) -> Result<Self, Error>
    where
        B: Behaviour,
        B::Spawn: Spawn<Handle = H>,
    {
        let mut inner: HttpHostInner<P, H> = config.try_into()?;
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

impl Default for HttpHost<TrivialParams, NoSpawnHandle> {
    fn default() -> Self {
        Self::new::<MinimalBehaviour>(HttpHostConfig::<TrivialKey>::default())
            .expect("Failed creating default HttpHost instance")
    }
}

impl<P: Params, H: Handle> Debug for HttpHost<P, H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("HttpHost")
            .field("base_url", &self.0.base_url)
            .finish()
    }
}

impl<P: Params, H: Handle> Clone for HttpHost<P, H> {
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
