use std::fmt::Debug;

use chrono::{DateTime, Utc};
use reqwest::{IntoUrl, Url};
use serde_json::Value;

use crate::measurement::{Channel, Measurement, PmSize, PmType};
use crate::sensor::{JsonMap, ReqwestSensor, Sensor};

#[derive(Debug)]
pub struct LanSensor {
    url: Url,
    live: bool,
}

impl LanSensor {
    pub fn new<T: IntoUrl>(url: T, live: bool) -> LanSensor {
        LanSensor {
            url: url
                .into_url()
                .expect("failed to parse URL")
                .join("json")
                .unwrap(),
            live,
        }
    }

    pub fn new_live_sensor<T: IntoUrl>(addr: T) -> LanSensor {
        LanSensor::new(addr, true)
    }

    pub fn new_average_sensor<T: IntoUrl>(addr: T) -> LanSensor {
        LanSensor::new(addr, false)
    }

    pub fn as_live(self) -> LanSensor {
        LanSensor::new_live_sensor(self.url)
    }

    pub fn as_average(self) -> LanSensor {
        LanSensor::new_average_sensor(self.url)
    }
}

impl Sensor for LanSensor {}

impl ReqwestSensor for LanSensor {
    type Measurement = LanMeasurement;

    fn construct_measurement(&self, json: JsonMap) -> LanMeasurement {
        LanMeasurement { json }
    }

    fn construct_url(&self) -> Url {
        if self.live {
            let mut url = self.url.clone();
            url.set_query(Some("live=true"));
            url
        } else {
            self.url.clone()
        }
    }
}

#[derive(Debug)]
pub struct LanMeasurement {
    json: JsonMap,
}

enum JsonType {
    F64,
    I64,
    String,
    U64,
}

impl LanMeasurement {
    fn get(&self, key: &str, expected_type: JsonType) -> &Value {
        // PurpleAir LAN JSON should be extremely consistent.
        // If a key is not found, that's panic-worthy.
        let value = self
            .json
            .get(key)
            .expect(&format!("PurpleAir LAN JSON is missing key: {}", key));
        match expected_type {
            JsonType::F64 => assert!(value.is_f64(), "{} is not a float, got: {:?}", key, value),
            JsonType::I64 => assert!(
                value.is_i64(),
                "{} is not an i64 int, got: {:?}",
                key,
                value
            ),
            JsonType::String => assert!(
                value.is_string(),
                "{} is not a string, got: {:?}",
                key,
                value
            ),
            JsonType::U64 => assert!(value.is_u64(), "{} is not a u64 int, got: {:?}", key, value),
        }
        value
    }

    fn get_string(&self, key: &str) -> String {
        String::from(self.get(key, JsonType::String).as_str().unwrap())
    }

    fn get_f64(&self, key: &str) -> f64 {
        self.get(key, JsonType::F64).as_f64().unwrap()
    }

    fn get_i64(&self, key: &str) -> i64 {
        self.get(key, JsonType::I64).as_i64().unwrap()
    }

    fn get_u64(&self, key: &str) -> u64 {
        self.get(key, JsonType::U64).as_u64().unwrap()
    }
}

impl Measurement for LanMeasurement {
    fn sensor_id(&self) -> String {
        self.get_string("SensorId")
    }

    fn timestamp(&self) -> DateTime<Utc> {
        let date_time = self.get_string("DateTime").to_uppercase().replace("/", "-");
        DateTime::parse_from_rfc3339(&date_time)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn latitude(&self) -> f64 {
        self.get_f64("lat")
    }

    fn longitude(&self) -> f64 {
        self.get_f64("lon")
    }

    fn place(&self) -> String {
        self.get_string("place")
    }

    fn rssi(&self) -> i64 {
        self.get_i64("rssi")
    }

    fn uptime(&self) -> u64 {
        self.get_u64("uptime")
    }

    fn temp_f(&self) -> i64 {
        self.get_i64("current_temp_f")
    }

    fn humidity(&self) -> i64 {
        self.get_i64("current_humidity")
    }

    fn dew_point_f(&self) -> i64 {
        self.get_i64("current_dewpoint_f")
    }

    fn pressure(&self) -> f64 {
        self.get_f64("pressure")
    }

    fn pm_2v5_aqi(&self, channel: Channel) -> Option<f64> {
        let key = format!("pm2.5_aqi{}", channel.string());
        Some(self.get_i64(&key) as f64)
    }

    fn particulate_mass(&self, pm_size: PmSize, pm_type: PmType, channel: Channel) -> Option<f64> {
        match pm_size {
            PmSize::Pm0v3 | PmSize::Pm0v5 | PmSize::Pm5v0 => return None,
            _ => (),
        }

        let key = format!(
            "pm{}_{}{}",
            pm_size.string(),
            pm_type.string(),
            channel.string()
        );
        Some(self.get_f64(&key))
    }

    fn particle_count(&self, pm_size: PmSize, channel: Channel) -> Option<f64> {
        let key = format!("p_{}_um{}", pm_size.string(), channel.string());
        Some(self.get_f64(&key))
    }
}
