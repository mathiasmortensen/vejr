CREATE TABLE locations (
    id UUID PRIMARY KEY,
    grid_key TEXT NOT NULL UNIQUE,

    requested_latitude DOUBLE PRECISION NOT NULL,
    requested_longitude DOUBLE PRECISION NOT NULL,

    model_latitude DOUBLE PRECISION NOT NULL,
    model_longitude DOUBLE PRECISION NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE weather_forecasts (
    location_id UUID NOT NULL
        REFERENCES locations(id)
        ON DELETE CASCADE,

    valid_at TIMESTAMPTZ NOT NULL,

    temperature_c DOUBLE PRECISION NOT NULL,
    feels_like_c DOUBLE PRECISION NOT NULL,

    humidity_percent DOUBLE PRECISION,
    cloud_cover_percent DOUBLE PRECISION,

    wind_speed_ms DOUBLE PRECISION,
    wind_direction_degrees DOUBLE PRECISION,
    wind_gust_ms DOUBLE PRECISION,

    rain_mm_per_hour DOUBLE PRECISION NOT NULL,
    snow_mm_per_hour_water_equivalent DOUBLE PRECISION NOT NULL,

    precipitation_type TEXT,
    lightning_probability_percent DOUBLE PRECISION,
    condition TEXT NOT NULL,

    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (location_id, valid_at)
);

CREATE INDEX weather_forecasts_location_fetched_idx
    ON weather_forecasts (location_id, fetched_at DESC);

CREATE INDEX weather_forecasts_valid_at_idx
    ON weather_forecasts (valid_at);