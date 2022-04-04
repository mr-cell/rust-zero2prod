FROM lukemathwalker/cargo-chef:latest-rust-1.54.0 AS planner

WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM lukemathwalker/cargo-chef:latest-rust-1.54.0 AS cacher

WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.54.0 AS build

WORKDIR /app
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY . .
ENV SQLX_OFFLINE true
RUN cargo build --release --bin rust-zero2prod

FROM node:17-alpine3.15 AS build-fe

WORKDIR /app
COPY frontend frontend
WORKDIR /app/frontend
RUN npm install
RUN npm run build

FROM debian:buster-slim AS run

WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/* \
COPY migrations migrations
COPY --from=build /app/target/release/rust-zero2prod rust-zero2prod
COPY --from=build-fe /app/static static
COPY templates templates
COPY configuration configuration
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./rust-zero2prod"]