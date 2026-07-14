use thiserror::Error;

use crate::dmi::models::DmiFeatureCollection;
use chrono::{Duration, SecondsFormat, Utc};
use reqwest::Client;

const DMI_FORECAST_URL: &str =
    "https://opendataapi.dmi.dk/v1/forecastedr/collections/harmonie_dini_sf/position";

const DMI_PARAMETERS: &str = concat!(
    "temperature-2m,",
    "relative-humidity-2m,",
    "wind-speed-10m,",
    "wind-dir-10m,",
    "gust-wind-speed-10m,",
    "fraction-of-cloud-cover,",
    "precipitation-type,",
    "rain-precipitation-rate,",
    "total-snowfall-rate-water-equivalent,",
    "probability-of-lightning"
);

#[derive(Clone)]
pub(crate) struct DmiClient {
    http_client: Client,
}

impl DmiClient {
    pub(crate) fn new(http_client: Client) -> Self {
        Self { http_client }
    }

    pub(crate) async fn fetch_forecast(
        &self,
        latitude: f64,
        longitude: f64,
    ) -> Result<DmiFeatureCollection, DmiError> {
        let now = Utc::now();

        let from = now - Duration::hours(1);
        let to = now + Duration::hours(48);

        let datetime = format!(
            "{}/{}",
            from.to_rfc3339_opts(SecondsFormat::Secs, true),
            to.to_rfc3339_opts(SecondsFormat::Secs, true)
        );

        let coordinates = format!("POINT({longitude} {latitude})");

        let query = [
            ("coords", coordinates),
            ("crs", "crs84".to_string()),
            ("parameter-name", DMI_PARAMETERS.to_string()),
            ("datetime", datetime),
            ("f", "GeoJSON".to_string()),
        ];

        let response = self
            .http_client
            .get(DMI_FORECAST_URL)
            .query(&query)
            .send()
            .await
            .map_err(DmiError::Request)?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "DMI returnerede ingen fejltekst".to_string());

            return Err(DmiError::Http {
                status: status.as_u16(),
                body,
            });
        }

        let forecast = response
            .json::<DmiFeatureCollection>()
            .await
            .map_err(DmiError::Decode)?;

        if forecast.features.is_empty() {
            return Err(DmiError::NoForecastData);
        }

        Ok(forecast)
    }
}

#[derive(Debug, Error)]
pub(crate) enum DmiError {
    #[error("Kunne ikke kontakte DMI: {0}")]
    Request(reqwest::Error),

    #[error("DMI returnerede HTTP {status}: {body}")]
    Http { status: u16, body: String },

    #[error("DMI returnerede JSON, som ikke kunne parses: {0}")]
    Decode(reqwest::Error),

    #[error("DMI returnerede ingen prognosedata")]
    NoForecastData,

    #[error("DMI returnerede ugyldige prognosedata: {0}")]
    InvalidData(String),
}
