# Build stage
FROM rust:1.75-slim as builder

WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libvirt-dev \
    libnvidia-ml-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libvirt0 \
    libnvidia-ml1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

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