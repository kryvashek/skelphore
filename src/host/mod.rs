#[cfg(feature = "callbacks")]
pub mod callbacks;
pub mod config;
#[cfg(test)]
mod tests;

use std::{
    convert::{TryFrom, TryInto},
    fmt::{Debug, Formatter, Result as FmtResult},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use const_format::formatcp;
pub use reqwest;
use reqwest::{Client, Method, RequestBuilder, Url};

use crate::{
    address::Address,
    credentials,
    timeoutsmap::{
        Params as TimeoutsParams, TimeoutsMap, TrivialKey, TrivialParams as TrivialTimeoutsParams,
    },
    Scheme,
};

#[cfg(feature = "pinger")]
use crate::ping::{self, pinger, Behaviour, Handling, MinimalBehaviour, NoHandling};

pub use self::config::*;

#[cfg(feature = "callbacks")]
pub use self::callbacks::*;

#[cfg(feature = "pinger")]
#[derive(Debug)]
pub enum PingState<H> {
    Config(ping::Config),
    Handle(H),
}

pub trait Params {
    type Timeouts: TimeoutsParams;
    #[cfg(feature = "pinger")]
    type Handling: Handling;
    #[cfg(feature = "callbacks")]
    type Callbacks: Callbacks;
}

pub struct TrivialParams;

impl Params for TrivialParams {
    type Timeouts = TrivialTimeoutsParams;
    #[cfg(feature = "pinger")]
    type Handling = NoHandling;
    #[cfg(feature = "callbacks")]
    type Callbacks = TrivialCallbacks;
}

struct HostInner<P: Params = TrivialParams> {
    client: Client,
    base_url: Url,
    timeouts: TimeoutsMap<P::Timeouts>,
    #[cfg(feature = "pinger")]
    ping: Option<PingState<<P::Handling as Handling>::Handle>>,
}

fn base_url(scheme: &'static str, instance: Address) -> Result<Url, Error> {
    let candidate = format!("{}://{}", scheme, instance);
    Url::from_str(&candidate).map_err(|source| Error::UrlParse { candidate, source })
}

impl<P: Params> HostInner<P> {
    pub fn new(config: HostConfig<<P::Timeouts as TimeoutsParams>::Key>) -> Result<Self, Error> {
        let HostConfig {
            credentials,
            target,
            scheme,
            timeouts,
            #[cfg(feature = "pinger")]
            ping,
            extras,
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

        if let Some(es) = extras {
            client = es.apply(client);
        }

        let client = client
            .https_only(matches!(scheme, Scheme::Https))
            .build()
            .map_err(Error::ClientBulid)?;

        let base_url = base_url(scheme.into(), target)?;

        Ok(Self {
            client,
            base_url,
            timeouts: TimeoutsMap::<P::Timeouts>::from(timeouts),
            #[cfg(feature = "pinger")]
            ping: ping.map(PingState::Config),
        })
    }

    fn url(&self, path: &str) -> Url {
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
        spec: Option<<P::Timeouts as TimeoutsParams>::Key>,
        xri: &str,
    ) -> RequestBuilder {
        let timeout = self.timeouts[spec.unwrap_or_default()];
        #[cfg(feature = "callbacks")]
        self.on_request_building(&method, path, timeout, Some(xri));
        self.request_builder(method, path, timeout)
            .header("X-Request-Id", xri)
    }

    #[cfg(feature = "pinger")]
    pub fn set_pinger<B: Behaviour<Handling = P::Handling>>(&mut self) -> bool {
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
        #[cfg(feature = "callbacks")]
        self.on_request_building(&method, &path, period, None);
        let request = self.request_builder(method, &path, period);
        self.ping = Some(PingState::Handle(pinger::<B>(request, period)));
        true
    }

    #[cfg(feature = "callbacks")]
    fn on_request_building(
        &self,
        method: &Method,
        path: &str,
        timeout: Duration,
        xri: Option<&str>,
    ) {
        P::Callbacks::on_request_building(&RequestInfo {
            method,
            path,
            timeout,
            xri,
        });
    }
}

#[cfg(feature = "pinger")]
impl<P: Params> Drop for HostInner<P> {
    fn drop(&mut self) {
        if let Some(PingState::Handle(handle)) = self.ping.take() {
            P::Handling::stop(handle)
        }
    }
}

impl<P: Params> TryFrom<HostConfig<<P::Timeouts as TimeoutsParams>::Key>> for HostInner<P> {
    type Error = Error;

    fn try_from(
        value: HostConfig<<P::Timeouts as TimeoutsParams>::Key>,
    ) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

pub struct Host<P: Params = TrivialParams>(Arc<HostInner<P>>);

impl<P: Params> Host<P> {
    #[cfg(feature = "pinger")]
    pub fn new<B: Behaviour<Handling = P::Handling>>(
        config: HostConfig<<P::Timeouts as TimeoutsParams>::Key>,
    ) -> Result<Self, Error> {
        let mut inner: HostInner<P> = config.try_into()?;
        inner.set_pinger::<B>();
        Ok(Self(Arc::new(inner)))
    }

    #[cfg(not(feature = "pinger"))]
    pub fn new(config: HostConfig<<P::Timeouts as TimeoutsParams>::Key>) -> Result<Self, Error> {
        Ok(Self(Arc::new(config.try_into()?)))
    }

    #[inline]
    pub fn post(
        &self,
        path: &str,
        spec: Option<<P::Timeouts as TimeoutsParams>::Key>,
        xri: &str,
    ) -> RequestBuilder {
        self.0.request(Method::POST, path, spec, xri)
    }

    #[inline]
    pub fn get(
        &self,
        path: &str,
        spec: Option<<P::Timeouts as TimeoutsParams>::Key>,
        xri: &str,
    ) -> RequestBuilder {
        self.0.request(Method::GET, path, spec, xri)
    }

    #[inline]
    pub fn request(
        &self,
        method: Method,
        path: &str,
        spec: Option<<P::Timeouts as TimeoutsParams>::Key>,
        xri: &str,
    ) -> RequestBuilder {
        self.0.request(method, path, spec, xri)
    }

    #[cfg(not(feature = "pinger"))]
    #[inline]
    pub fn ping(&self, method: Method, path: &str, timeout: Duration) -> RequestBuilder {
        self.0.request_builder(method, path, timeout)
    }
}

#[cfg(feature = "pinger")]
impl Default for Host<TrivialParams> {
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

impl<P: Params> Debug for Host<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Host")
            .field("base_url", &self.0.base_url)
            .finish()
    }
}

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
