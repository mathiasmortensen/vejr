use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct DmiFeatureCollection {
    pub features: Vec<DmiFeature>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DmiFeature {
    pub geometry: DmiGeometry,
    pub properties: DmiProperties,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DmiGeometry {
    pub coordinates: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DmiProperties {
    pub step: DateTime<Utc>,

    #[serde(flatten)]
    values: HashMap<String, Value>,
}

impl DmiProperties {
    pub(crate) fn number(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(value_to_f64)
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),

        Value::String(value) => value.parse::<f64>().ok(),

        Value::Array(values) => values.first().and_then(value_to_f64),

        _ => None,
    }
}
