CREATE TABLE measurements (
     id SERIAL PRIMARY KEY,
     experiment_id VARCHAR NOT NULL,
     timestamp DOUBLE PRECISION NOT NULL,
     temperature DOUBLE PRECISION NOT NULL,
     out_of_range BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_temperature_experiment_id_timestamp ON measurements(experiment_id, timestamp);
CREATE INDEX idx_temperature_experiment_id_out_of_range ON measurements(experiment_id, out_of_range);
