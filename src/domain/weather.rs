use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherResponse {
    pub location: WeatherLocation,
    pub current: ForecastPoint,
    pub hourly: Vec<ForecastPoint>,
    pub source: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherLocation {
    pub requested: Coordinates,
    pub model_point: Coordinates,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastPoint {
    pub valid_at: DateTime<Utc>,

    pub temperature_c: f64,
    pub feels_like_c: f64,

    pub humidity_percent: Option<f64>,
    pub cloud_cover_percent: Option<f64>,

    pub wind_speed_ms: Option<f64>,
    pub wind_direction_degrees: Option<f64>,
    pub wind_gust_ms: Option<f64>,

    pub rain_mm_per_hour: f64,
    pub snow_mm_per_hour_water_equivalent: f64,

    pub precipitation_type: Option<PrecipitationType>,
    pub lightning_probability_percent: Option<f64>,

    pub condition: WeatherCondition,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrecipitationType {
    Drizzle,
    Rain,
    Sleet,
    Snow,
    FreezingDrizzle,
    FreezingRain,
    Graupel,
    Hail,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Drizzle,
    Rain,
    HeavyRain,
    Sleet,
    Snow,
    FreezingDrizzle,
    FreezingRain,
    Graupel,
    Hail,
    Thunderstorm,
}
