FROM rust:1.70.0 as builder

RUN apt-get update \
    && apt-get install -y cmake ca-certificates

WORKDIR /usr/src/myapp

COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/microservice /usr/local/bin/microservice
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

CMD ["microservice"]