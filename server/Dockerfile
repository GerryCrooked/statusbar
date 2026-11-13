FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.toml
COPY src src
COPY static static
COPY locales locales
RUN cargo build --release

FROM debian:bookworm-slim
COPY ca/ /usr/local/share/ca-certificates/
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && update-ca-certificates && rm -rf /var/lib/apt/lists/* && useradd -r -u 10001 -m app && mkdir /data && chown app:app /data
USER app
COPY --from=build /src/target/release/vg-status-server /usr/local/bin/vg-status-server
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/vg-status-server"]
