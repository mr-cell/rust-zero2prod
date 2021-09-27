FROM lukemathwalker/cargo-chef:latest-rust-1.54.0 AS planner

WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM lukemathwalker/cargo-chef:latest-rust-1.54.0 AS cacher

WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.54.0 AS build

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

WORKDIR /app
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY . .
ENV SQLX_OFFLINE true
RUN cargo build --target x86_64-unknown-linux-musl --release --bin rust-zero2prod

FROM scratch

WORKDIR /app
COPY migrations migrations
COPY configuration configuration
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/rust-zero2prod rust-zero2prod
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./rust-zero2prod"]