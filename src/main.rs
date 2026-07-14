mod dmi;
mod domain;

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    dmi::{DmiClient, DmiError, normalize_forecast},
    domain::weather::WeatherResponse,
};

use sqlx::PgPool;

#[derive(Clone)]
struct AppState {
    dmi_client: DmiClient,
    database: PgPool,
}

#[derive(Debug, Deserialize)]
struct ForecastQuery {
    lat: f64,
    lon: f64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    dotenvy::dotenv().ok();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL skal være sat");

    let database = PgPool::connect(&database_url)
        .await
        .expect("Kunne ikke forbinde til PostgreSQL");

    sqlx::migrate!()
        .run(&database)
        .await
        .expect("Kunne ikke køre databasemigrations");

    let http_client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("dmi-weather-app/0.1")
        .build()
        .expect("Kunne ikke oprette HTTP-klient");

    let state = AppState {
        dmi_client: DmiClient::new(http_client),
        database,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/forecast", get(forecast))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Kunne ikke starte serveren");

    tracing::info!("Serveren kører på http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("Serverfejl");
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok"
    }))
}

async fn forecast(
    State(state): State<AppState>,
    Query(query): Query<ForecastQuery>,
) -> Result<Json<WeatherResponse>, AppError> {
    validate_coordinates(query.lat, query.lon)?;

    let raw_forecast = state
        .dmi_client
        .fetch_forecast(query.lat, query.lon)
        .await?;

    let normalized = normalize_forecast(raw_forecast, query.lat, query.lon)?;

    Ok(Json(normalized))
}

fn validate_coordinates(latitude: f64, longitude: f64) -> Result<(), AppError> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err(AppError::InvalidCoordinates(
            "Koordinater skal være gyldige tal".to_string(),
        ));
    }

    if !(-90.0..=90.0).contains(&latitude) {
        return Err(AppError::InvalidCoordinates(
            "Latitude skal være mellem -90 og 90".to_string(),
        ));
    }

    if !(-180.0..=180.0).contains(&longitude) {
        return Err(AppError::InvalidCoordinates(
            "Longitude skal være mellem -180 og 180".to_string(),
        ));
    }

    Ok(())
}

#[derive(Debug, Error)]
enum AppError {
    #[error("{0}")]
    InvalidCoordinates(String),

    #[error(transparent)]
    Dmi(#[from] DmiError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidCoordinates(_) => StatusCode::BAD_REQUEST,
            Self::Dmi(_) => StatusCode::BAD_GATEWAY,
        };

        (
            status,
            Json(json!({
                "error": self.to_string()
            })),
        )
            .into_response()
    }
}