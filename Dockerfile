# Build stage
FROM rust:1.75-slim-bookworm as builder

WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libvirt-dev \
    libnvidia-ml-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
RUN cargo install cargo-chef
COPY Cargo.toml Cargo.lock .
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code
COPY . .

# Build application
RUN cargo build --release --locked

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libvirt0 \
    libnvidia-ml1 \
    libvirt-clients \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -ms /bin/bash appuser
WORKDIR /app
RUN chown appuser:appuser /app
USER appuser

# Copy the built executable
COPY --from=builder /usr/src/app/target/release/gpu-share-vm-manager .
# Copy configuration
COPY config /app/config

# Create necessary directories
RUN mkdir -p /var/lib/gpu-share/images

# Set environment variables
ENV CONFIG_PATH=/app/config
ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["./gpu-share-vm-manager"]
CMD ["serve"]