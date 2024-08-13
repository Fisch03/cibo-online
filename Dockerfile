FROM rustlang/rust:nightly-slim AS rust

ENV SQLX_OFFLINE=true
ENV SKIP_CLIENT_BUILD=true

# install shared tools
RUN cargo install cargo-chef 
WORKDIR /usr/src/cibo-online

FROM rust AS plan
# prepare deps
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM rust AS build-web
# install tools
RUN rustup target add wasm32-unknown-unknown
RUN cargo install wasm-pack

# compile web client
COPY . .
RUN wasm-pack build --target web --release ./web_client

FROM rust AS build-server
# compile/cache deps
COPY --from=plan /usr/src/cibo-online/recipe.json recipe.json
COPY rust-toolchain.toml rust-toolchain.toml
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
# get compiled web client
COPY --from=build-web /usr/src/cibo-online/web_client/pkg web_client/pkg
# compile server
RUN cargo build --bin cibo_online-server --release


FROM debian:bookworm-slim AS runtime
WORKDIR /cibo-online
COPY .env .env
COPY --from=build-server /usr/src/cibo-online/target/release/cibo_online-server cibo_online-server
COPY --from=build-server /usr/src/cibo-online/static static

EXPOSE 8080
EXPOSE 8081
CMD ["./cibo_online-server"]