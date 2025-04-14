FROM rust:1.86 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir -p /data

COPY . .
RUN cargo build --release
 
EXPOSE 7700

CMD ["./target/release/sndra-link"]