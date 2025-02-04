# Build stage
FROM rust:1.75 as builder

WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libvirt-dev \
    libnvidia-ml-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
RUN cargo install cargo-chef
COPY Cargo.toml Cargo.lock ./
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code
COPY . ./

# Build application
RUN cargo build --release --locked

# Runtime stage
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/gpu-share-vm-manager /app/
CMD ["/app/gpu-share-vm-manager"]