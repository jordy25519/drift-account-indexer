FROM rust:1.70 as builder
WORKDIR /build
COPY ./ /build
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /build/target/release/indexer /bin/indexer
ENTRYPOINT indexer