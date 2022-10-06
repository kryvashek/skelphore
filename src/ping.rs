use reqwest::{Method, RequestBuilder, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{convert::Infallible, fmt::Display, future::Future, marker::PhantomData, time::Duration};

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(with = "humantime_serde", default = "Config::def_period")]
    pub period: Duration,
    pub path: String,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default = "Config::def_method")]
    pub method: Method,
}

impl Config {
    pub fn def_period() -> Duration {
        Duration::from_secs(4)
    }

    pub fn def_method() -> Method {
        Method::GET
    }
}

pub trait Question: Serialize + Sized {
    fn ask() -> Option<Self>;
}

#[derive(Serialize)]
pub struct EmptyQuestion;

impl Question for EmptyQuestion {
    fn ask() -> Option<Self> {
        None
    }
}

pub trait Answer: DeserializeOwned {
    type Fail: Display;

    fn positivness(self) -> Result<(), Self::Fail>;
}

#[derive(Deserialize)]
pub struct EmptyAnswer;

impl Answer for EmptyAnswer {
    type Fail = Infallible;

    fn positivness(self) -> Result<(), Self::Fail> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Sleep {
    async fn sleep(duration: Duration);
}

pub struct DontSleep;

#[async_trait::async_trait]
impl Sleep for DontSleep {
    async fn sleep(_duration: Duration) {}
}

pub trait ProcessError<R: Display> {
    fn process_ping_error(error: Error<R>);
    fn process_request_clone_fail();
}

pub struct DontProcessError<R: Display>(PhantomData<R>);

impl<R: Display> ProcessError<R> for DontProcessError<R> {
    fn process_ping_error(_error: Error<R>) {}
    fn process_request_clone_fail() {}
}

pub trait Handle {
    type Output;

    fn stop(&self);
}

pub struct NoSpawnHandle;

impl Handle for NoSpawnHandle {
    type Output = ();

    fn stop(&self) {}
}

pub trait Spawn {
    type Handle: Handle;

    fn spawn<Fut>(f: Fut) -> Self::Handle
    where
        Fut: Future<Output = <Self::Handle as Handle>::Output> + 'static;
}

pub struct DontSpawn;

impl Spawn for DontSpawn {
    type Handle = NoSpawnHandle;

    fn spawn<Fut>(_: Fut) -> Self::Handle
    where
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        NoSpawnHandle
    }
}

pub trait Behaviour: 'static {
    type Question: Question;
    type Answer: Answer;
    type Sleep: Sleep;
    type ProcessError: ProcessError<<<Self as Behaviour>::Answer as Answer>::Fail>;
    type Spawn: Spawn;
}

pub struct MinimalBehaviour;

impl Behaviour for MinimalBehaviour {
    type Question = EmptyQuestion;
    type Answer = EmptyAnswer;
    type Sleep = DontSleep;
    type ProcessError = DontProcessError<<EmptyAnswer as Answer>::Fail>;
    type Spawn = DontSpawn;
}

async fn ping_once<Q: Question, A: Answer>(
    mut request: RequestBuilder,
) -> Result<(), Error<A::Fail>> {
    if let Some(question) = Q::ask() {
        request = request.json(&question);
    };
    let response = request.send().await.map_err(Error::Request)?;
    let status = response.status();
    let positivness_result = response
        .json::<A>()
        .await
        .map_err(Error::Response)?
        .positivness();
    match (status.is_success(), positivness_result) {
        (_, Err(result)) => Err(Error::NegativeResult { status, result }),
        (false, Ok(_)) => Err(Error::NegativeStatus(status)),
        (true, Ok(_)) => Ok(()),
    }
}

pub fn pinger<B: Behaviour>(
    request: RequestBuilder,
    period: Duration,
) -> <<B as Behaviour>::Spawn as Spawn>::Handle {
    B::Spawn::spawn(async move {
        let mut current_period = period;
        loop {
            let request_clone = match request.try_clone() {
                None => {
                    B::ProcessError::process_request_clone_fail();
                    B::Sleep::sleep(period).await;
                    continue;
                }
                Some(x) => x,
            };
            match ping_once::<B::Question, B::Answer>(request_clone).await {
                Err(ping_error) => {
                    B::ProcessError::process_ping_error(ping_error);
                    current_period += period;
                }
                Ok(_) => current_period = period,
            }
            B::Sleep::sleep(current_period).await;
        }
    })
}

#[derive(Debug, thiserror::Error)] // NOTE: impossible to derive from Clone because reqwest::Error doesn't implement it
pub enum Error<R: Display> {
    #[error("Failed sending ping request: {0}")]
    Request(reqwest::Error),
    #[error("Failed receiving ping response: {0}")]
    Response(reqwest::Error),
    #[error("Negative ping result with status {status}: {result}")]
    NegativeResult { status: StatusCode, result: R },
    #[error("Negative ping status {0}")]
    NegativeStatus(StatusCode),
}
