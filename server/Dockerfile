FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.* ./
COPY src src
COPY static static
COPY locales locales
RUN cargo build --release

FROM debian:bookworm-slim
# Optional: drop PEM CA certificates (*.crt) into server/ca/ to trust an internal LDAP CA.
COPY ca/ /usr/local/share/ca-certificates/
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && update-ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -r -u 10001 app && mkdir /data && chown app:app /data
USER app
COPY --from=build /src/target/release/statusbar-server /usr/local/bin/statusbar-server
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/statusbar-server"]
