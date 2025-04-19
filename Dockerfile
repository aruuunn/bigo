# Dockerfile
FROM rust:slim-bullseye

WORKDIR /app

RUN apt update -y && apt install -y unzip curl

RUN curl -LO https://github.com/protocolbuffers/protobuf/releases/download/v27.3/protoc-27.3-linux-x86_64.zip
RUN unzip protoc-27.3-linux-x86_64.zip -d /usr/local
RUN chmod +x /usr/local/bin/protoc

# Cache dependencies first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
RUN rm -r src

# Copy actual source`
COPY . .

RUN cargo build --release

CMD ["./target/release/rs"]
