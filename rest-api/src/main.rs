use actix_web::{web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use actix_web::web::Data;
use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct OutOfRangeQuery {
    experiment_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TemperatureQuery {
    experiment_id: String,
    start_time: f64,
    end_time: f64,
}

struct AppState {
    pool: Pool,
}

async fn out_of_range(query: web::Query<OutOfRangeQuery>, data: web::Data<AppState>) -> HttpResponse {
    eprintln!("Out of range query for {}", &query.experiment_id);

    let client = data.pool.get().await.unwrap();

    let stmt = client.prepare_cached(
        "SELECT timestamp, temperature FROM measurements WHERE experiment_id = $1 AND out_of_range"
    ).await.unwrap();
    let rows = client
        .query(&stmt, &[&query.experiment_id])
        .await
        .unwrap();

    let mut result = Vec::new();
    for row in rows {
        result.push(serde_json::json!({
            "timestamp": row.get::<_, f64>("timestamp"),
            "temperature": row.get::<_, f64>("temperature"),
        }));
    }

    HttpResponse::Ok().json(result)
}

async fn temperature(query: web::Query<TemperatureQuery>, data: web::Data<AppState>) -> HttpResponse {
    eprintln!("Temperature query for {} start {} end {}", &query.experiment_id, &query.start_time, &query.end_time);

    let client = data.pool.get().await.unwrap();

    let stmt = client.prepare_cached(
        "SELECT timestamp, temperature FROM measurements WHERE experiment_id = $1 AND timestamp BETWEEN $2 AND $3"
    ).await.unwrap();
    let rows = client
        .query(&stmt, &[&query.experiment_id, &query.start_time, &query.end_time])
        .await
        .unwrap();

    let mut result = Vec::new();
    for row in rows {
        result.push(serde_json::json!({
            "timestamp": row.get::<_, f64>("timestamp"),
            "temperature": row.get::<_, f64>("temperature"),
        }));
    }

    HttpResponse::Ok().json(result)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure the database pool
    let cfg = Config {
        host: Some("db".into()),
        user: Some("admin".into()),
        password: Some("secret".into()),
        dbname: Some("postgres".into()),
        ..Config::default()
    };

    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(AppState {
                pool: pool.clone(),
            }))
            .route("/temperature/out-of-range", web::get().to(out_of_range))
            .route("/temperature", web::get().to(temperature))
    })
        .bind("0.0.0.0:3003")?
        .run()
        .await
}
