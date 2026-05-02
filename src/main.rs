use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HaState {
    entity_id: String,
    state: String,
    attributes: serde_json::Value,
}

const ROOM_SENSORS: &[&str] = &[
    "sensor.back_bedroom_temperature",
    "sensor.dining_room_sensor_temperature",
    "sensor.front_bedroom_sensor_temperature",
    "sensor.garage_sensor_temperature",
    "sensor.living_room_sensor_temperature",
    "sensor.main_bedroom_sensor_temperature",
    "sensor.office_room_sensor_temperature",
    "sensor.upstairs_hallway_sensor_temperature",
];

#[tokio::main]
async fn main() -> Result<()> {
    let ha_url = std::env::var("HA_URL")
        .context("Missing HA_URL env var, e.g. http://homeassistant.local:8123")?;

    let token = std::env::var("HA_TOKEN").context("Missing HA_TOKEN env var")?;

    let client = Client::new();

    println!("Fetching room temperatures from Home Assistant...\n");

    for entity_id in ROOM_SENSORS {
        let url = format!("{}/api/states/{}", ha_url.trim_end_matches('/'), entity_id);

        let response = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .with_context(|| format!("Failed to request {}", entity_id))?;

        if !response.status().is_success() {
            println!("{}: ERROR {}", entity_id, response.status());
            continue;
        }

        let state: HaState = response
            .json()
            .await
            .with_context(|| format!("Failed to parse response for {}", entity_id))?;

        let friendly_name = state
            .attributes
            .get("friendly_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&state.entity_id);

        let unit = state
            .attributes
            .get("unit_of_measurement")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        println!("{:<40} {:>6} {}", friendly_name, state.state, unit);
    }

    Ok(())
}
