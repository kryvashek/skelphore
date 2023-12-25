use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    time::Duration,
};

use reqwest::Method;

#[derive(Clone, Debug)]
pub struct RequestInfo<'a> {
    pub method: &'a Method,
    pub path: &'a str,
    pub timeout: Duration,
    pub xri: Option<&'a str>,
}

impl Display for RequestInfo<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        cubob::StructShow::inherit(f)
            .field(&"method", &self.method)
            .field(&"path", &self.path)
            .field(
                &"timeout",
                &humantime_serde::re::humantime::format_duration(self.timeout),
            )
            .field_opt(&"xri", &self.xri)
            .finish()
    }
}

pub trait Callbacks {
    fn on_request_building(request_info: &RequestInfo);
}

pub struct TrivialCallbacks;

impl Callbacks for TrivialCallbacks {
    fn on_request_building(_request_info: &RequestInfo) {}
}
