# Dockerfile
FROM rust:bullseye

WORKDIR /app

RUN apt update -y && apt install -y protobuf-compiler

# Cache dependencies first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
RUN rm -r src

# Copy actual source`
COPY . .

RUN cargo build --release

CMD ["./target/release/rs"]
