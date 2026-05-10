use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HaState {
    state: String,
    attributes: serde_json::Value,
}

struct RoomConfig {
    name: &'static str,
    sensor: &'static str,
    trvs: &'static [&'static str],
    include: bool,
}

const ROOMS: &[RoomConfig] = &[
    RoomConfig {
        name: "Back Bedroom",
        sensor: "sensor.back_bedroom_temperature",
        trvs: &["climate.back_bedroom_thermostat"],
        include: true,
    },
    RoomConfig {
        name: "Front Bedroom",
        sensor: "sensor.front_bedroom_sensor_temperature",
        trvs: &["climate.front_bedroom_thermostat_2"],
        include: true,
    },
    RoomConfig {
        name: "Main Bedroom",
        sensor: "sensor.main_bedroom_sensor_temperature",
        trvs: &["climate.main_bedroom_thermostat"],
        include: true,
    },
    RoomConfig {
        name: "Living Room",
        sensor: "sensor.living_room_sensor_temperature",
        trvs: &[
            "climate.living_room_thermostat_1",
            "climate.living_room_thermostat_2",
        ],
        include: true,
    },
    RoomConfig {
        name: "Dining Room",
        sensor: "sensor.dining_room_sensor_temperature",
        trvs: &["climate.front_room_thermostat"],
        include: true,
    },
    RoomConfig {
        name: "Office",
        sensor: "sensor.office_room_sensor_temperature",
        trvs: &["climate.office_room_thermostat"],
        include: true,
    },
    RoomConfig {
        name: "Upstairs Hallway",
        sensor: "sensor.upstairs_hallway_sensor_temperature",
        trvs: &["climate.hallway_thermostat"],
        include: true,
    },
    RoomConfig {
        name: "Garage",
        sensor: "sensor.garage_sensor_temperature",
        trvs: &[],
        include: false,
    },
];

async fn fetch_state(
    client: &Client,
    ha_url: &str,
    token: &str,
    entity_id: &str,
) -> Result<HaState> {
    let url = format!("{}/api/states/{}", ha_url.trim_end_matches('/'), entity_id);

    let response = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .with_context(|| format!("Failed to request {}", entity_id))?;

    if !response.status().is_success() {
        anyhow::bail!("{} returned {}", entity_id, response.status());
    }

    response
        .json()
        .await
        .with_context(|| format!("Failed to parse response for {}", entity_id))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let ha_url = std::env::var("HA_URL")
        .context("Missing HA_URL env var, e.g. http://homeassistant.local:8123")?;

    let token = std::env::var("HA_TOKEN").context("Missing HA_TOKEN env var")?;

    let client = Client::new();

    let mut room_results: Vec<(&str, f64, String, f64, &str)> = Vec::new();

    for room in ROOMS {
        let sensor_state = fetch_state(&client, &ha_url, &token, room.sensor).await?;

        let actual_temp: f64 = sensor_state
            .state
            .parse()
            .with_context(|| format!("Invalid temperature for {}", room.sensor))?;

        let mut targets = Vec::new();

        for trv in room.trvs {
            let trv_state = fetch_state(&client, &ha_url, &token, trv).await?;

            if let Some(target) = trv_state
                .attributes
                .get("temperature")
                .and_then(|v| v.as_f64())
            {
                targets.push(target);
            }
        }

        let avg_target = if targets.is_empty() {
            None
        } else {
            Some(targets.iter().sum::<f64>() / targets.len() as f64)
        };

        let target_display = match avg_target {
            Some(target) => format!("{:.1}°C", target),
            None => "n/a".to_string(),
        };

        let demand = match avg_target {
            Some(target) => (target - actual_temp).max(0.0),
            None => 0.0,
        };

        let status = if demand > 0.5 {
            "HIGH DEMAND"
        } else if demand > 0.2 {
            "LOW DEMAND"
        } else {
            "SATISFIED"
        };

        room_results.push((room.name, actual_temp, target_display, demand, status));
    }

    println!("Heating IHD - Room Summary\n");

    for (name, actual_temp, target_display, demand, status) in &room_results {
        println!(
            "{:<18} actual: {:>5.1}°C | target: {:>6} | demand: {:>4.1}°C | {}",
            name, actual_temp, target_display, demand, status
        );
    }

    let total_demand: f64 = room_results.iter().map(|r| r.3).sum();

    let rooms_calling = room_results.iter().filter(|r| r.3 > 0.2).count();

    let heating_rooms: Vec<_> = room_results
        .iter()
        .zip(ROOMS.iter())
        .filter(|(_, config)| config.include)
        .map(|(result, _)| result)
        .collect();

    let coldest_room = heating_rooms
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .expect("At least one room should exist");

    let heating_on = total_demand > 0.5;

    println!("\n================================");
    println!("🔥 HEATING SUMMARY");
    println!("================================");
    println!("Total Demand:     {:.2}°C", total_demand);
    println!("Rooms Calling:    {}", rooms_calling);
    println!(
        "Coldest Room:     {} ({:.1}°C)",
        coldest_room.0, coldest_room.1
    );
    println!(
        "Heating Decision: {}",
        if heating_on { "ON" } else { "OFF" }
    );

    Ok(())
}
