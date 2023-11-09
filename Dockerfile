FROM rust:1.71.0-bookworm AS chef

ENV CARGO_REGISTRIES_CARTESI_INDEX=https://github.com/cartesi/crates-index
RUN rustup component add rustfmt
RUN cargo install cargo-chef

FROM chef AS planner
WORKDIR /app
COPY . /app/
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --test cartesi_machine_client_tests

FROM --platform=linux/amd64 cartesi/playground:0.6.0
USER root

COPY --from=builder /app/target/release/ /root/target/release/
COPY ./ /root
WORKDIR /root

# CMD "/root/entrypoint.sh"
ENTRYPOINT [ "/root/entrypoint.sh" ]
