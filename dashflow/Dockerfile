# DashFlow Rust Production Dockerfile
# Multi-stage build for minimal production image

# Stage 1: Build environment
FROM rust:1.81-slim-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary
RUN cargo build --release --bin dashflow-server

# Stage 2: Runtime environment
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 dashflow && \
    mkdir -p /app && \
    chown -R dashflow:dashflow /app

WORKDIR /app
USER dashflow

# Copy binary from builder
COPY --from=builder --chown=dashflow:dashflow /build/target/release/dashflow-server /app/dashflow-server

# Health check endpoint
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/dashflow-server", "health"]

# Expose port
EXPOSE 8080

# Set environment
ENV RUST_LOG=info
ENV LANGCHAIN_ENV=production

# Run the binary
ENTRYPOINT ["/app/dashflow-server"]
CMD ["serve"]
