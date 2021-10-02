use chrono::{DateTime, Utc};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

fn f_to_c(f: i64) -> f64 {
    (f as f64 - 32.0) * 5.0 / 9.0
}

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

pub trait Measurement {
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

    ///
    /// Run the EPA correction on purpleair sensors
    ///
    ///  Ref:
    ///     - https://cfpub.epa.gov/si/si_public_record_report.cfm?Lab=CEMM&dirEntryId=349513
    ///
    /// Note:
    ///     This doesn't run the 1-hour averages that are recommended as the
    ///     measurement class only deals with current readings.
    ///
    /// Note:
    ///     The web sensor has the possibility of null-values.
    ///
    /// Args:
    ///     pm2v5_cf_1_a: (float) Channel A reading of pm2.5 concentration with CF 1
    ///     pm2v5_cf_1_b: (float) Channel B reading of pm2.5 concentration with CF 1
    ///     humidity: (float) Current humidity measured by the sensor.
    ///
    /// Returns:
    ///    corrected pm2.5 value
    fn get_epa_correction(pm2v5_cf_1_a: f64, pm2v5_cf_1_b: f64, humidity: i64) -> f64 {
        // Using the equation on page 8 of the EPA report pdf
        // constants on that page are different than at the end for some reason.
        let pm2v5_mean = (pm2v5_cf_1_a + pm2v5_cf_1_b) / 2.0;
        // It's possible when pm2v5 is near zero and the humidty is high that pm2.5
        // could go negative after correction. Assume anything less than zero is zero.
        (0.0 as f64).max(0.52 * pm2v5_mean - 0.085 * (humidity as f64) + 5.71)
    }

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

    fn pm_2v5_aqi_epa(&self) -> Option<f64> {
        match self.pm_2v5_epa_correction() {
            Some(value) => Some(Self::get_aqi(value)),
            None => None,
        }
    }

    fn sensor_id(&self) -> String;

    fn timestamp(&self) -> DateTime<Utc>;

    fn latitude(&self) -> f64;

    fn longitude(&self) -> f64;

    fn place(&self) -> String;

    fn rssi(&self) -> i64;

    fn uptime(&self) -> u64;

    fn temp_f(&self) -> i64;

    fn temp_c(&self) -> f64 {
        f_to_c(self.temp_f())
    }

    fn humidity(&self) -> i64;

    fn dew_point_f(&self) -> i64;

    fn dew_point_c(&self) -> f64 {
        f_to_c(self.dew_point_f())
    }

    fn pressure(&self) -> f64;

    fn pm_2v5_aqi(&self, channel: Channel) -> Option<f64>;

    fn particulate_mass(&self, pm_size: PmSize, pm_type: PmType, channel: Channel) -> Option<f64>;

    fn particle_count(&self, pm_size: PmSize, channel: Channel) -> Option<f64>;
}
