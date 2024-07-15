FROM rust:latest

RUN cargo install wasm-pack

WORKDIR /usr/src/cibo-online
COPY . .

RUN cargo install --path .

CMD ["cibo_online-runner"]

