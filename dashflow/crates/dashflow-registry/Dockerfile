# DashFlow Registry Server - Production Dockerfile
#
# Multi-stage build for minimal final image size.
#
# Build:
#   docker build -t dashflow-registry -f crates/dashflow-registry/Dockerfile .
#
# Run:
#   docker run -p 3030:3030 \
#     -e DATABASE_URL=postgres://user:pass@host/db \
#     dashflow-registry

# ============================================================================
# Stage 1: Build
# ============================================================================
FROM rust:1.82-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/dashflow-registry/Cargo.toml crates/dashflow-registry/

# Create dummy source files for dependency compilation
RUN mkdir -p crates/dashflow-registry/src && \
    echo "fn main() {}" > crates/dashflow-registry/src/main.rs && \
    echo "pub fn lib() {}" > crates/dashflow-registry/src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release --features server,postgres -p dashflow-registry 2>/dev/null || true

# Copy actual source code
COPY crates/dashflow-registry crates/dashflow-registry/

# Rebuild with actual source
RUN touch crates/dashflow-registry/src/lib.rs crates/dashflow-registry/src/bin/registry_server.rs && \
    cargo build --release --features server,postgres -p dashflow-registry --bin registry_server

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -u 1000 -U dashflow

# Create data directories
RUN mkdir -p /data/packages && chown -R dashflow:dashflow /data

# Copy binary from builder
COPY --from=builder /build/target/release/registry_server /usr/local/bin/

# Switch to non-root user
USER dashflow

# Default environment
ENV REGISTRY_HOST=0.0.0.0 \
    REGISTRY_PORT=3030 \
    RUST_LOG=dashflow_registry=info

# Expose port
EXPOSE 3030

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:${REGISTRY_PORT}/health || exit 1

# Run server
ENTRYPOINT ["/usr/local/bin/registry_server"]
