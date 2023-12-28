use std::time::Duration;

use super::*;

use crate::{
    address::Address,
    credentials::Credentials,
    timeoutsmap::tests::{Spec, SpecParams},
    Scheme,
};

pub struct HostParams;

impl Params for HostParams {
    type Timeouts = SpecParams;
    #[cfg(feature = "pinger")]
    type Handling = NoHandling;
    #[cfg(feature = "callbacks")]
    type Callbacks = TrivialCallbacks;
    const USER_AGENT: &'static str = formatcp!(
        "{}-test/{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
}

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
        Address::new("example.com", 4321).expect("Address should be created as 'example.com:4321'")
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

    let _ = Host::<HostParams>::new::<MinimalBehaviour>(config)
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
        Address::new("example.com", 4321).expect("Address should be created as 'example.com:4321'")
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

    let _ = Host::<HostParams>::new(config)
        .expect("Host instance should be created from config smoothly");
}
