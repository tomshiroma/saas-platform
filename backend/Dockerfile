FROM rust:1.94-bookworm AS development

RUN apt-get update \
    && apt-get install --yes --no-install-recommends curl \
    && cargo install cargo-watch --locked \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build \
    && rm -rf src

COPY --chown=app:app . .

USER app

EXPOSE 3000

CMD ["cargo", "watch", "-x", "run"]

FROM rust:1.94-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app
COPY --from=build /app/target/release/saas-api /usr/local/bin/saas-api
USER app
EXPOSE 3000
CMD ["saas-api"]
