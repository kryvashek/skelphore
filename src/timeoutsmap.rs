use humantime_serde::Serde;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
    time::Duration,
};

pub trait Key: Eq + Hash + Into<usize> + Default {}

#[derive(PartialEq, Eq, Hash, Default, Deserialize)]
pub struct TrivialKey;

impl From<TrivialKey> for usize {
    fn from(_: TrivialKey) -> Self {
        0
    }
}

impl Key for TrivialKey {}

pub trait Array: IndexMut<usize, Output = Duration> {
    fn new(default: Duration) -> Self;
}

pub type UsualArray<const N: usize> = [Duration; N];

impl<const N: usize> Array for UsualArray<N> {
    fn new(default: Duration) -> Self {
        [default; N]
    }
}

pub type TrivialArray = UsualArray<1>;

pub trait Params {
    type Key: Key;
    type Array: Array;
}

pub struct TrivialParams;

impl Params for TrivialParams {
    type Key = TrivialKey;
    type Array = TrivialArray;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutsMapConfig<K: Key = TrivialKey> {
    #[serde(
        with = "humantime_serde",
        default = "default_timeouts_map_config_default"
    )]
    pub default: Duration,
    #[serde(flatten)]
    pub map: HashMap<K, Serde<Duration>>,
}

fn default_timeouts_map_config_default() -> Duration {
    Duration::from_millis(120)
}

impl<K: Key> TimeoutsMapConfig<K> {
    #[cfg(test)]
    pub fn only_default(default_ms: u64) -> Self {
        Self {
            default: Duration::from_millis(default_ms),
            map: HashMap::default(),
        }
    }

    pub fn def_default() -> Duration {
        default_timeouts_map_config_default()
    }
}

impl<K: Key> Default for TimeoutsMapConfig<K> {
    fn default() -> Self {
        Self {
            default: Self::def_default(),
            map: HashMap::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimeoutsMap<P: Params = TrivialParams>(P::Array);

impl<P: Params> From<TimeoutsMapConfig<P::Key>> for TimeoutsMap<P> {
    fn from(TimeoutsMapConfig { default, map }: TimeoutsMapConfig<P::Key>) -> Self {
        let mut this = Self(P::Array::new(default));
        map.into_iter()
            .for_each(|(spec, duration)| this.0[spec.into()] = duration.into_inner());
        this
    }
}

impl<P: Params> Index<P::Key> for TimeoutsMap<P> {
    type Output = Duration;

    fn index(&self, spec: P::Key) -> &Self::Output {
        &self.0[spec.into()]
    }
}

#[cfg(test)]
pub mod tests {
    use enum_iterator::IntoEnumIterator;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, IntoEnumIterator)]
    #[serde(rename_all = "lowercase")]
    #[repr(u8)]
    pub enum Spec {
        Undefined = 0,
        Alice,
        Bob,
        Charlie,
        Duncan,
    }

    impl From<Spec> for usize {
        fn from(src: Spec) -> Self {
            src as usize
        }
    }

    impl Key for Spec {}

    impl Default for Spec {
        fn default() -> Self {
            Spec::Undefined
        }
    }

    pub struct SpecParams;

    impl Params for SpecParams {
        type Key = Spec;
        type Array = UsualArray<{ Spec::ITEM_COUNT }>;
    }

    const CONFIG_TEXT: &str = r#"
    default = "111ms"
    "alice" = "222ms"
    charlie = "333ms""#;

    #[test]
    fn config_read_and_apply() {
        let config: TimeoutsMapConfig<Spec> =
            toml::from_str(CONFIG_TEXT).expect("Config should deserialize smoothly");
        let timeouts = TimeoutsMap::<SpecParams>::from(config);

        assert_eq!(timeouts[Spec::Alice], Duration::from_millis(222));
        assert_eq!(timeouts[Spec::Charlie], Duration::from_millis(333));

        assert_eq!(timeouts[Spec::Undefined], Duration::from_millis(111));
        assert_eq!(timeouts[Spec::Bob], Duration::from_millis(111));
        assert_eq!(timeouts[Spec::Duncan], Duration::from_millis(111));
    }
}
