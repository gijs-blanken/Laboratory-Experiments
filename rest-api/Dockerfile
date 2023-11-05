FROM rust:1.70.0 as builder

WORKDIR /usr/src/myapp

COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/rest-api /usr/local/bin/rest-api
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

CMD ["rest-api"]