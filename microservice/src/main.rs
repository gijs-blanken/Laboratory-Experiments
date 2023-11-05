mod structs;

use std::collections::HashMap;
use std::option::Option;

use apache_avro::Reader;
use apache_avro::from_value;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
use structs::{ExperimentConfig};
use apache_avro::types::Value;
use crate::structs::{Database, ExperimentStarted, ExperimentTerminated, NotificationType, SensorTemperatureMeasured, StabilizationStarted};
use crate::structs::ExperimentPhase::*;

async fn consume_kafka_messages() {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "microservice_consumer")
        .set("bootstrap.servers", "13.49.128.80:19093,13.49.128.80:29093,13.49.128.80:39093")
        .set("auto.offset.reset", "latest")
        .set("security.protocol", "SSL")
        .set("ssl.ca.location", "./auth/ca.crt")
        .set("ssl.keystore.location", "./auth/kafka.keystore.pkcs12")
        .set("ssl.keystore.password", "cc2023")
        .set("enable.auto.commit", "true")
        .set("ssl.endpoint.identification.algorithm", "none")
        .create()
        .expect("Consumer creation failed");

    consumer.subscribe(&["experiment"]).expect("Can't subscribe to specified topic");

    let mut configs: HashMap<String, ExperimentConfig> = HashMap::new();
    let mut temperature_map: HashMap<String, Vec<f64>> = HashMap::new();
    let db = Database::new().await.expect("Failed to connect to the database");

    loop {
        match consumer.recv().await {
            Err(e) => println!("Kafka error: {}", e),
            Ok(m) => {
                let event_type = match m.headers() {
                    Some(headers) => String::from_utf8(Vec::from(headers.get(0).unwrap().1)).unwrap_or_default(),
                    None => String::default(),
                };

                let reader = match m.payload() {
                    Some(a) => Reader::new(&a[..]).unwrap(),
                    None => continue
                };

                for value in reader {
                    handle_event(&event_type, &value.unwrap(), &mut configs, &mut temperature_map, &db).await;
                }
            }
        }
    }
}

async fn handle_event(event_type: &str, event: &Value, configs: &mut HashMap<String, ExperimentConfig>, temperature_map: &mut HashMap<String, Vec<f64>>, db: &Database) {
    match event_type {
        "experiment_configured" => {
            let config: ExperimentConfig = from_value(event).unwrap();
            configs.insert(config.experiment.clone(), config.clone());
            eprintln!("Experiment configured {:?}", &config);
        }
        "stabilization_started" => {
            let stabilization: StabilizationStarted = from_value(event).unwrap();
            if let Some(config) = configs.get_mut(&*stabilization.experiment) {
                config.phase = Stabilization;
                eprintln!("Experiment stabilization phase started {}", &config.experiment);
                return;
            }

            eprintln!("Unknown experiment stabilization phase started {}", &stabilization.experiment);
        }
        "experiment_started" => {
            let experiment: ExperimentStarted = from_value(event).unwrap();
            if let Some(config) = configs.get_mut(&*experiment.experiment) {
                config.phase = Active;
                eprintln!("Experiment started {}", &config.experiment);
                return;
            }

            eprintln!("Unknown experiment stabilization phase started {}", &experiment.experiment);
        }
        "sensor_temperature_measured" => {
            let sensor_temp: SensorTemperatureMeasured = from_value(event).unwrap();
            if let Some(config) = configs.get_mut(&*sensor_temp.experiment) {
                handle_temperature_measured(config, &sensor_temp, temperature_map, db).await;
                eprintln!("Sensor measurement for {} from {:?} with temp {}", &config.experiment, &sensor_temp.sensor, &sensor_temp.temperature);
                return;
            }

            eprintln!("Unknown experiment sensor measurement {}", &sensor_temp.experiment);
        }
        "experiment_terminated" => {
            let termination: ExperimentTerminated = from_value(event).unwrap();
            configs.remove(&*termination.experiment);
            eprintln!("Experiment terminated {}", &termination.experiment);
        }
        _ => eprintln!("Unknown event type: {}", event_type),
    }
}

async fn handle_temperature_measured(config: &mut ExperimentConfig, measurement: &SensorTemperatureMeasured, temperature_map: &mut HashMap<String, Vec<f64>>, db: &Database) {
    let average_temp_option: Option<f64> = get_average_temp(config, temperature_map, measurement.temperature);

    match average_temp_option {
        None => {}
        Some(average_temp) => {
            let out_of_range = average_temp > config.temperature_range.upper_threshold ||
                average_temp < config.temperature_range.lower_threshold;

            if config.phase != Stabilization && config.phase != Configuration {
                db.insert_temperature_data(config.experiment.clone(), measurement.timestamp, average_temp, out_of_range).await;
            }

            match config.phase {
                Configuration => {}
                StabilizationNotified => {}
                Stabilization => if !out_of_range {
                    config.phase = StabilizationNotified;
                    notify_service(NotificationType::Stabilized, config, measurement);
                },
                Active => if out_of_range {
                    config.phase = OutOfRange;
                    notify_service(NotificationType::OutOfRange, config, measurement);
                    return;
                },
                OutOfRange => if !out_of_range {
                    config.phase = Active;
                },
            }
        }
    }
}

fn get_average_temp(config: &ExperimentConfig, temperature_map: &mut HashMap<String, Vec<f64>>, new_temperature: f64) -> Option<f64> {
    match temperature_map.get_mut(&*config.experiment) {
        None => {
            temperature_map.insert(config.experiment.clone(), vec!(new_temperature));
            None
        }
        Some(temperatures) => {
            if temperatures.len() + 1 >= config.sensors.len() {
                temperatures.push(new_temperature);
                let average_temp: f64 = temperatures.iter().sum::<f64>() / temperatures.len() as f64;
                temperatures.clear();

                return Some(average_temp);
            }

            temperatures.push(new_temperature);
            None
        }
    }
}

#[tokio::main]
async fn main() {
    consume_kafka_messages().await;
}

fn notify_service(notification_type: NotificationType, experiment_config: &ExperimentConfig, measurement: &SensorTemperatureMeasured) {
    let payload = serde_json::json!({
        "notification_type": notification_type,
        "researcher": experiment_config.researcher,
        "experiment_id": experiment_config.experiment,
        "measurement_id": measurement.measurement_id,
        "cipher_data": measurement.measurement_hash,
    });

    tokio::spawn(async move {
        let url = format!("https://notifications-service.cc2023.4400app.me/api/notify?token={}", "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJleHAiOjE3MDQxMjk4MTgsInN1YiI6Imdyb3VwMSJ9.ZcG2mdluk1ruskpA2kbhwhwFR4573hx-diI_JA_90iexIig0ubGAOcPjxgQnu3ALlhoF6fBgN3fxbu5Hp369JfavkWlVNva8EjiVh3wOKNV_CO1yiN0U79-eZF1v3z6aAL4KajQ7JE5I6ru9f1Rze6EB4GoPuX_jiUh0TlaKxMBpfrBh6Gg22Ck133kyVuG0L2vhIzJlPs3h3doc5fbhDPGXmad5FcntaTZNmz-5IyxckBLg6vkYaBIkx4xyNhwhog-mIcUNZ1HnNt6HRSg89v-PptkiQ-g7D9AuCrktXaIXalPgToHkDk1e2749zuPSoP4g4xRRFQhixPAICUIZzA");
        let _ = reqwest::Client::new()
            .post(url)
            .json(&payload)
            .send().await;
    });
}

