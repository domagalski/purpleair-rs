use chrono::{DateTime, Utc};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

fn f_to_c(f: i64) -> f64 {
    (f as f64 - 32.0) * 5.0 / 9.0
}

/// Particulate Mass size.
#[derive(Clone, Copy, Debug, EnumIter)]
pub enum PmSize {
    Pm0v3,
    Pm0v5,
    Pm1v0,
    Pm2v5,
    Pm5v0,
    Pm10v0,
}

impl PmSize {
    pub fn string(&self) -> String {
        match self {
            PmSize::Pm0v3 => String::from("0_3"),
            PmSize::Pm0v5 => String::from("0_5"),
            PmSize::Pm1v0 => String::from("1_0"),
            PmSize::Pm2v5 => String::from("2_5"),
            PmSize::Pm5v0 => String::from("5_0"),
            PmSize::Pm10v0 => String::from("10_0"),
        }
    }
}

/// Particulate Mass correction factor type.
#[derive(Clone, Copy, Debug, EnumIter)]
pub enum PmType {
    Atm,
    Cf1,
}

impl PmType {
    pub fn string(&self) -> String {
        match self {
            PmType::Atm => String::from("atm"),
            PmType::Cf1 => String::from("cf_1"),
        }
    }
}

/// PurpleAir sensor channel.
#[derive(Clone, Copy, Debug, EnumIter)]
pub enum Channel {
    A,
    B,
}

impl Channel {
    pub fn string(&self) -> String {
        match self {
            Channel::A => String::new(),
            Channel::B => String::from("_b"),
        }
    }
}

/// Measurement of all data from a PurpleAir sensor.
///
/// It is recommended to use the EPA corrections, as the PurpleAir sensors can sometimes
/// overestimate PM 2.5 and therefore overestimate AQI. See the following paper for the EPA
/// corrections: 
///
/// <https://cfpub.epa.gov/si/si_public_record_report.cfm?Lab=CEMM&dirEntryId=349513>
///
/// See the following for an explanation on Air Quality Index:
///
/// <https://en.wikipedia.org/wiki/Air_quality_index>
pub trait Measurement {
    /// Get the Air Quality Index (AQI) for a given PM 2.5 value.
    ///
    /// Args:
    /// * `pm_2v5`: The PM 2.5 value to use to determine the AQI.
    ///
    /// Returns:
    ///     AQI value from the PM 2.5 value.
    fn get_aqi(pm_2v5: f64) -> f64 {
        //NOTE: since the table on wikipedia is ambiguous to what happens at
        //jump points if there is more than a decimal of precision. Therefore,
        //we multiplying pm2.5 by 10 and convert to and int for detecting the
        //concentration limits of the pm2.5 value.
        let pm_2v5: i32 = (10.0 * pm_2v5) as i32;

        //Taken from wikipedia: https://en.wikipedia.org/wiki/Air_quality_index
        static CONCENTRATION_LIMITS: &[(i32, i32)] = &[
            (0, 120),
            (121, 354),
            (355, 554),
            (555, 1504),
            (1505, 2504),
            (2505, 3504),
            (3505, 5004),
        ];

        static AQI_LIMITS: &[(i32, i32)] = &[
            (0, 50),
            (51, 100),
            (101, 150),
            (151, 200),
            (201, 300),
            (301, 400),
            (401, 500),
        ];

        assert_eq!(CONCENTRATION_LIMITS.len(), AQI_LIMITS.len());

        let (idx, (c_low, c_high)) = CONCENTRATION_LIMITS
            .iter()
            .enumerate()
            .filter(|(_, (low, high))| pm_2v5 >= *low && pm_2v5 <= *high)
            .next()
            .unwrap_or((
                CONCENTRATION_LIMITS.len() - 1,
                CONCENTRATION_LIMITS.last().unwrap(),
            ));
        let c_low = *c_low as f64 / 10.0;
        let c_high = *c_high as f64 / 10.0;
        let i_low = AQI_LIMITS[idx].0 as f64;
        let i_high = AQI_LIMITS[idx].1 as f64;
        let pm_2v5 = pm_2v5 as f64 / 10.0;
        (i_high - i_low) * (pm_2v5 - c_low) / (c_high - c_low) + i_low
    }

    /// Run the EPA correction on purpleair sensors
    ///
    /// Ref: <https://cfpub.epa.gov/si/si_public_record_report.cfm?Lab=CEMM&dirEntryId=349513>
    ///
    /// Note:
    ///     This doesn't run the 1-hour averages that are recommended as the
    ///     measurement class only deals with current readings.
    ///
    /// Args:
    /// * `pm2v5_cf_1_a`: Channel A reading of pm2.5 concentration with CF 1
    /// * `pm2v5_cf_1_b`: Channel B reading of pm2.5 concentration with CF 1
    /// * `humidity`: Current humidity measured by the sensor.
    ///
    /// Returns:
    ///     Corrected pm2.5 value
    fn get_epa_correction(pm2v5_cf_1_a: f64, pm2v5_cf_1_b: f64, humidity: i64) -> f64 {
        // Using the equation on page 8 of the EPA report pdf
        // constants on that page are different than at the end for some reason.
        let pm2v5_mean = (pm2v5_cf_1_a + pm2v5_cf_1_b) / 2.0;
        // It's possible when pm2v5 is near zero and the humidty is high that pm2.5
        // could go negative after correction. Assume anything less than zero is zero.
        (0.0 as f64).max(0.52 * pm2v5_mean - 0.085 * (humidity as f64) + 5.71)
    }

    /// Get the EPA correction for the PM 2.5 values from the measurement reading.
    ///
    /// Note:
    ///     The web sensor has the possibility of null-values.
    fn pm_2v5_epa_correction(&self) -> Option<f64> {
        let pm_2v5_cf_1: Vec<f64> = Channel::iter()
            .map(|ch| self.particulate_mass(PmSize::Pm2v5, PmType::Cf1, ch))
            .filter(|value| value.is_some())
            .map(|value| value.unwrap())
            .collect();

        if pm_2v5_cf_1.len() != 2 {
            return None;
        }

        Some(Self::get_epa_correction(
            pm_2v5_cf_1[0],
            pm_2v5_cf_1[1],
            self.humidity(),
        ))
    }

    /// Get the EPA-corrected AQI from the measurement reading.
    ///
    /// Note:
    ///     The web sensor has the possibility of null-values.
    fn pm_2v5_aqi_epa(&self) -> Option<f64> {
        match self.pm_2v5_epa_correction() {
            Some(value) => Some(Self::get_aqi(value)),
            None => None,
        }
    }

    /// The ID of the PurpleAir Sensor.
    fn sensor_id(&self) -> String;

    /// The timestamp of the measurement reading.
    fn timestamp(&self) -> DateTime<Utc>;

    /// The latitude of the sensor.
    fn latitude(&self) -> f64;

    /// The longitude of the sensor.
    fn longitude(&self) -> f64;

    /// Whether the sensor is inside/outside.
    fn place(&self) -> String;

    /// Wi-Fi RSSI value (dBm).
    fn rssi(&self) -> i64;

    /// Sensor uptime in seconds.
    fn uptime(&self) -> u64;

    /// Sensor temperature in Fahrenheit.
    fn temp_f(&self) -> i64;

    /// Sensor temperature in Celsius.
    fn temp_c(&self) -> f64 {
        f_to_c(self.temp_f())
    }

    /// Sensor humidity.
    fn humidity(&self) -> i64;

    /// Sensor dew point in Fahrenheit.
    fn dew_point_f(&self) -> i64;

    /// Sensor dew point in Celsius.
    fn dew_point_c(&self) -> f64 {
        f_to_c(self.dew_point_f())
    }

    /// Sensor pressure in in millibars.
    fn pressure(&self) -> f64;

    /// PM 2.5 AQI for a sensor channel.
    fn pm_2v5_aqi(&self, channel: Channel) -> Option<f64>;

    /// Get the Particulate Mass (PM) for a particle size.
    ///
    /// Args:
    /// * `pm_size`: The particle size for the PM value.
    /// * `pm_type`: The PM correction factor (ATM, CF=1).
    /// * `channel`: The PurpleAir sensor channel.
    ///
    /// Note:
    ///     The web sensor has the possibility of null-values.
    fn particulate_mass(&self, pm_size: PmSize, pm_type: PmType, channel: Channel) -> Option<f64>;

    /// Get the Particle Count for a particle size.
    ///
    /// Args:
    /// * `pm_size`: The particle size for the PM value.
    /// * `channel`: The PurpleAir sensor channel.
    ///
    /// Note:
    ///     The web sensor has the possibility of null-values.
    fn particle_count(&self, pm_size: PmSize, channel: Channel) -> Option<f64>;
}
