use serde::{Deserialize, Serialize};
use tokio::spawn;
use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

#[derive(Debug, Deserialize)]
pub struct Event {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ExperimentPhase {
    Configuration,
    Stabilization,
    StabilizationNotified,
    Active,
    OutOfRange,
}

impl Default for ExperimentPhase {
    fn default() -> Self {
        ExperimentPhase::Configuration
    }
}

#[derive(Debug, Serialize)]
pub enum NotificationType {
    Stabilized,
    OutOfRange,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExperimentConfig {
    pub experiment: String,
    pub researcher: String,
    pub sensors: Vec<String>,
    pub temperature_range: TemperatureRange,
    #[serde(default)]
    pub phase: ExperimentPhase,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemperatureRange {
    pub upper_threshold: f64,
    pub lower_threshold: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StabilizationStarted {
    pub experiment: String,
    pub timestamp: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExperimentStarted {
    pub experiment: String,
    pub timestamp: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorTemperatureMeasured {
    pub experiment: String,
    pub sensor: String,
    pub measurement_id: String,
    pub timestamp: f64,
    pub temperature: f64,
    pub measurement_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExperimentTerminated {
    pub experiment: String,
    pub timestamp: f64,
}

pub struct Database {
    pool: Pool,
}

impl Database {
    pub (crate) async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let cfg = Config {
            host: Some("db".into()),
            user: Some("admin".into()),
            password: Some("secret".into()),
            dbname: Some("postgres".into()),
            ..Config::default()
        };

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();

        Ok(Database { pool })
    }

    pub async fn insert_temperature_data(&self, experiment: String, timestamp: f64, temperature: f64, out_of_range: bool) {
        let pool = self.pool.clone();
        spawn(async move {
            let client = pool.get().await.expect("Error getting client from the pool");
            let stmt = client.prepare_cached(
                "INSERT INTO measurements (experiment_id, timestamp, temperature, out_of_range) VALUES ($1, $2, $3, $4)"
            ).await.unwrap();
            let _ = client.execute(
                &stmt,&[&experiment, &timestamp, &temperature, &out_of_range],
            ).await.expect("Error inserting data into the database");

            eprintln!("Inserted into the database experiment {} {} {} {}", experiment, timestamp, temperature, out_of_range);
        });
    }
}
