use humantime_serde::Serde;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut},
    time::Duration,
};

#[derive(PartialEq, Eq, Hash, Default, Deserialize)]
pub struct TrivialKey;

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
    type Key: Eq + Hash + Default;
    type Array: Array;

    fn key_as_usize(key: &Self::Key) -> usize;
}

pub struct TrivialParams;

impl Params for TrivialParams {
    type Key = TrivialKey;
    type Array = TrivialArray;

    fn key_as_usize(_: &Self::Key) -> usize {
        0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutsMapConfig<K: Eq + Hash + Default = TrivialKey> {
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

impl<K: Eq + Hash + Default> TimeoutsMapConfig<K> {
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

impl<K: Eq + Hash + Default> Default for TimeoutsMapConfig<K> {
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
            .for_each(|(spec, duration)| this.0[P::key_as_usize(&spec)] = duration.into_inner());
        this
    }
}

impl<P: Params> Index<P::Key> for TimeoutsMap<P> {
    type Output = Duration;

    fn index(&self, spec: P::Key) -> &Self::Output {
        &self.0[P::key_as_usize(&spec)]
    }
}

#[cfg(test)]
pub mod tests {
    use enum_iterator::Sequence;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Sequence, Default)]
    #[serde(rename_all = "lowercase")]
    #[repr(u8)]
    pub enum Spec {
        #[default]
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

    pub struct SpecParams;

    impl Params for SpecParams {
        type Key = Spec;
        type Array = UsualArray<{ Spec::CARDINALITY }>;

        fn key_as_usize(key: &Self::Key) -> usize {
            *key as usize
        }
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
