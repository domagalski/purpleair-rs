use reqwest::{blocking, Result as RqResult, Url};
use serde_json::{Map, Value};

pub type JsonMap = Map<String, Value>;

pub trait ReqwestSensor {
    type Measurement;

    fn construct_measurement(&self, json: JsonMap) -> Self::Measurement;

    fn construct_url(&self) -> Url;
}

/// PurpleAir sensor abstraction.
pub trait Sensor: ReqwestSensor {
    /// Read a measurement from the PurpleAir sensor.
    fn get_measurement(&self) -> RqResult<Self::Measurement> {
        let url = self.construct_url();
        let json = blocking::get(url)?.json::<JsonMap>()?;
        Ok(self.construct_measurement(json))
    }
}
