use chrono::Utc;

use crate::dmi::{
    DmiError,
    models::{DmiFeature, DmiFeatureCollection, DmiProperties},
};
use crate::domain::weather::{
    Coordinates, ForecastPoint, PrecipitationType, WeatherCondition, WeatherLocation,
    WeatherResponse,
};

const MINIMUM_PRECIPITATION_MM_PER_HOUR: f64 = 0.1;
const THUNDERSTORM_PROBABILITY_PERCENT: f64 = 50.0;
const HEAVY_RAIN_MM_PER_HOUR: f64 = 4.0;

pub(crate) fn normalize_forecast(
    raw: DmiFeatureCollection,
    requested_latitude: f64,
    requested_longitude: f64,
) -> Result<WeatherResponse, DmiError> {
    let mut normalized = raw
        .features
        .into_iter()
        .filter_map(|feature| match normalize_feature(feature) {
            Ok(feature) => Some(feature),

            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Ignorerer ugyldigt prognosepunkt fra DMI"
                );

                None
            }
        })
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        return Err(DmiError::NoForecastData);
    }

    normalized.sort_by_key(|feature| feature.forecast.valid_at);

    let now = Utc::now();

    let current = normalized
        .iter()
        .min_by_key(|feature| (feature.forecast.valid_at - now).num_seconds().abs())
        .map(|feature| feature.forecast.clone())
        .ok_or(DmiError::NoForecastData)?;

    let model_point = normalized
        .first()
        .map(|feature| feature.coordinates.clone())
        .ok_or(DmiError::NoForecastData)?;

    let hourly = normalized
        .into_iter()
        .map(|feature| feature.forecast)
        .filter(|forecast| forecast.valid_at >= now)
        .take(24)
        .collect::<Vec<_>>();

    Ok(WeatherResponse {
        location: WeatherLocation {
            requested: Coordinates {
                latitude: requested_latitude,
                longitude: requested_longitude,
            },
            model_point,
        },
        current,
        hourly,
        source: "DMI HARMONIE DINI SF".to_string(),
        generated_at: now,
    })
}

struct NormalizedFeature {
    coordinates: Coordinates,
    forecast: ForecastPoint,
}

fn normalize_feature(feature: DmiFeature) -> Result<NormalizedFeature, DmiError> {
    if feature.geometry.coordinates.len() < 2 {
        return Err(DmiError::InvalidData(
            "GeoJSON-punkt mangler koordinater".to_string(),
        ));
    }

    let longitude = feature.geometry.coordinates[0];
    let latitude = feature.geometry.coordinates[1];

    if !longitude.is_finite() || !latitude.is_finite() {
        return Err(DmiError::InvalidData(
            "GeoJSON-punkt indeholder ugyldige koordinater".to_string(),
        ));
    }

    let temperature_kelvin = required_number(&feature.properties, "temperature-2m")?;

    let temperature_c = kelvin_to_celsius(temperature_kelvin);

    let humidity_percent = optional_number(&feature.properties, "relative-humidity-2m")
        .map(|value| round(value.clamp(0.0, 100.0), 1));

    let wind_speed_ms = optional_number(&feature.properties, "wind-speed-10m")
        .map(|value| round(value.max(0.0), 1));

    let wind_direction_degrees = optional_number(&feature.properties, "wind-dir-10m")
        .map(normalize_degrees)
        .map(|value| round(value, 0));

    let wind_gust_ms = optional_number(&feature.properties, "gust-wind-speed-10m")
        .map(|value| round(value.max(0.0), 1));

    let cloud_cover_percent = optional_number(&feature.properties, "fraction-of-cloud-cover")
        .map(fraction_to_percent)
        .map(|value| round(value, 0));

    let lightning_probability_percent =
        optional_number(&feature.properties, "probability-of-lightning")
            .map(fraction_to_percent)
            .map(|value| round(value, 0));

    let rain_mm_per_hour = optional_number(&feature.properties, "rain-precipitation-rate")
        .map(rate_to_mm_per_hour)
        .unwrap_or(0.0);

    let snow_mm_per_hour_water_equivalent =
        optional_number(&feature.properties, "total-snowfall-rate-water-equivalent")
            .map(rate_to_mm_per_hour)
            .unwrap_or(0.0);

    let precipitation_type = optional_number(&feature.properties, "precipitation-type")
        .and_then(parse_precipitation_type);

    let condition = determine_condition(
        precipitation_type,
        rain_mm_per_hour,
        snow_mm_per_hour_water_equivalent,
        cloud_cover_percent,
        lightning_probability_percent,
    );

    let feels_like_c = calculate_feels_like(temperature_c, humidity_percent, wind_speed_ms);

    Ok(NormalizedFeature {
        coordinates: Coordinates {
            latitude,
            longitude,
        },

        forecast: ForecastPoint {
            valid_at: feature.properties.step,

            temperature_c: round(temperature_c, 1),
            feels_like_c: round(feels_like_c, 1),

            humidity_percent,
            cloud_cover_percent,

            wind_speed_ms,
            wind_direction_degrees,
            wind_gust_ms,

            rain_mm_per_hour: round(rain_mm_per_hour, 2),
            snow_mm_per_hour_water_equivalent: round(snow_mm_per_hour_water_equivalent, 2),

            precipitation_type,
            lightning_probability_percent,

            condition,
        },
    })
}

fn required_number(properties: &DmiProperties, key: &str) -> Result<f64, DmiError> {
    optional_number(properties, key)
        .ok_or_else(|| DmiError::InvalidData(format!("Den nødvendige parameter '{key}' mangler")))
}

fn optional_number(properties: &DmiProperties, key: &str) -> Option<f64> {
    properties.number(key).filter(|value| value.is_finite())
}

fn kelvin_to_celsius(kelvin: f64) -> f64 {
    kelvin - 273.15
}

fn fraction_to_percent(fraction: f64) -> f64 {
    fraction.clamp(0.0, 1.0) * 100.0
}

fn rate_to_mm_per_hour(rate_kg_m2_second: f64) -> f64 {
    rate_kg_m2_second.max(0.0) * 3600.0
}

fn normalize_degrees(degrees: f64) -> f64 {
    ((degrees % 360.0) + 360.0) % 360.0
}

fn parse_precipitation_type(value: f64) -> Option<PrecipitationType> {
    match value.round() as i32 {
        0 => Some(PrecipitationType::Drizzle),
        1 => Some(PrecipitationType::Rain),
        2 => Some(PrecipitationType::Sleet),
        3 => Some(PrecipitationType::Snow),
        4 => Some(PrecipitationType::FreezingDrizzle),
        5 => Some(PrecipitationType::FreezingRain),
        6 => Some(PrecipitationType::Graupel),
        7 => Some(PrecipitationType::Hail),
        _ => None,
    }
}

fn determine_condition(
    precipitation_type: Option<PrecipitationType>,
    rain_mm_per_hour: f64,
    snow_mm_per_hour: f64,
    cloud_cover_percent: Option<f64>,
    lightning_probability_percent: Option<f64>,
) -> WeatherCondition {
    let precipitation = rain_mm_per_hour + snow_mm_per_hour;

    let lightning_probability = lightning_probability_percent.unwrap_or(0.0);

    if precipitation >= MINIMUM_PRECIPITATION_MM_PER_HOUR
        && lightning_probability >= THUNDERSTORM_PROBABILITY_PERCENT
    {
        return WeatherCondition::Thunderstorm;
    }

    if precipitation >= MINIMUM_PRECIPITATION_MM_PER_HOUR {
        return match precipitation_type {
            Some(PrecipitationType::Drizzle) => WeatherCondition::Drizzle,

            Some(PrecipitationType::Rain) if rain_mm_per_hour >= HEAVY_RAIN_MM_PER_HOUR => {
                WeatherCondition::HeavyRain
            }

            Some(PrecipitationType::Rain) => WeatherCondition::Rain,

            Some(PrecipitationType::Sleet) => WeatherCondition::Sleet,

            Some(PrecipitationType::Snow) => WeatherCondition::Snow,

            Some(PrecipitationType::FreezingDrizzle) => WeatherCondition::FreezingDrizzle,

            Some(PrecipitationType::FreezingRain) => WeatherCondition::FreezingRain,

            Some(PrecipitationType::Graupel) => WeatherCondition::Graupel,

            Some(PrecipitationType::Hail) => WeatherCondition::Hail,

            None if snow_mm_per_hour > rain_mm_per_hour => WeatherCondition::Snow,

            None => WeatherCondition::Rain,
        };
    }

    match cloud_cover_percent.unwrap_or(0.0) {
        cloud_cover if cloud_cover >= 85.0 => WeatherCondition::Cloudy,

        cloud_cover if cloud_cover >= 25.0 => WeatherCondition::PartlyCloudy,

        _ => WeatherCondition::Clear,
    }
}

fn calculate_feels_like(
    temperature_c: f64,
    humidity_percent: Option<f64>,
    wind_speed_ms: Option<f64>,
) -> f64 {
    let humidity = humidity_percent.unwrap_or(50.0);
    let wind_speed = wind_speed_ms.unwrap_or(0.0);

    let wind_speed_kmh = wind_speed * 3.6;

    // Wind chill anvendes kun ved køligt og tilstrækkeligt
    // blæsende vejr.
    if temperature_c <= 10.0 && wind_speed_kmh >= 4.8 {
        let wind_factor = wind_speed_kmh.powf(0.16);

        let wind_chill = 13.12 + 0.6215 * temperature_c - 11.37 * wind_factor
            + 0.3965 * temperature_c * wind_factor;

        return wind_chill.min(temperature_c);
    }

    // Heat index anvendes ved varmt og fugtigt vejr.
    if temperature_c >= 26.7 && humidity >= 40.0 {
        return calculate_heat_index(temperature_c, humidity).max(temperature_c);
    }

    temperature_c
}

fn calculate_heat_index(temperature_c: f64, humidity_percent: f64) -> f64 {
    let temperature_f = temperature_c * 9.0 / 5.0 + 32.0;
    let humidity = humidity_percent;

    let heat_index_f = -42.379 + 2.049_015_23 * temperature_f + 10.143_331_27 * humidity
        - 0.224_755_41 * temperature_f * humidity
        - 0.006_837_83 * temperature_f.powi(2)
        - 0.054_817_17 * humidity.powi(2)
        + 0.001_228_74 * temperature_f.powi(2) * humidity
        + 0.000_852_82 * temperature_f * humidity.powi(2)
        - 0.000_001_99 * temperature_f.powi(2) * humidity.powi(2);

    (heat_index_f - 32.0) * 5.0 / 9.0
}

fn round(value: f64, decimal_places: u32) -> f64 {
    let factor = 10_f64.powi(decimal_places as i32);

    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn normalizes_dmi_forecast() {
        let raw: DmiFeatureCollection = serde_json::from_value(json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": [12.5683, 55.6761]
                    },
                    "properties": {
                        "step": "2026-07-14T10:00:00Z",
                        "temperature-2m": 291.15,
                        "relative-humidity-2m": 70.0,
                        "wind-speed-10m": 4.0,
                        "wind-dir-10m": 220.0,
                        "gust-wind-speed-10m": 8.0,
                        "fraction-of-cloud-cover": 0.75,
                        "rain-precipitation-rate": 0.0002777778,
                        "total-snowfall-rate-water-equivalent": 0.0,
                        "probability-of-lightning": 0.20,
                        "precipitation-type": 1
                    }
                }
            ]
        }))
        .expect("Testdata skal kunne parses");

        let normalized =
            normalize_forecast(raw, 55.6761, 12.5683).expect("Prognosen skal kunne normaliseres");

        assert_eq!(normalized.current.temperature_c, 18.0);

        assert!((normalized.current.rain_mm_per_hour - 1.0).abs() < 0.01);

        assert_eq!(normalized.current.cloud_cover_percent, Some(75.0));

        assert_eq!(normalized.current.lightning_probability_percent, Some(20.0));

        assert_eq!(normalized.current.condition, WeatherCondition::Rain);
    }
}
