# =============================================================================
# CogniArk - AI Agent Docker Image
# =============================================================================
#
# RECOMMENDED: Use docker-compose for automatic data persistence
#
#   ./start.sh          (Linux/Mac)
#   start.bat           (Windows)
#   docker-compose up -d --build
#
# Your data (conversations, actions, settings) is automatically preserved
# across rebuilds when using docker-compose.
#
# =============================================================================
# MANUAL DOCKER RUN (must include volumes to preserve data):
#
#   docker run -d -p 17990:17990 \
#     -v cogniark-data:/app/data \
#     -v cogniark-config:/app/config \
#     --name cogniark \
#     cogniark:latest
#
# WARNING: Running without -v volumes will LOSE YOUR DATA on container removal!
# =============================================================================

# Build stage
FROM rust:1.85-bookworm AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Pin dependencies that require too-new Rust versions
RUN cargo update home@0.5.12 --precise 0.5.9 || true && \
    cargo update time@0.3.46 --precise 0.3.36 || true && \
    cargo update time-core@0.1.8 --precise 0.1.2 || true && \
    cargo update time-macros@0.2.26 --precise 0.2.18 || true

RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
COPY config ./config
COPY actions ./actions
COPY assets ./assets

# Build for release
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create non-root user
RUN useradd -m -u 1000 agent
RUN mkdir -p /app/data /app/data/actions /app/config && chown -R agent:agent /app

# Copy binary and assets
COPY --from=builder /app/target/release/cogniark /app/
COPY --from=builder /app/config /app/config
COPY --from=builder /app/actions /app/actions
COPY --from=builder /app/assets /app/assets

# Fix permissions for all directories
RUN chown -R agent:agent /app/config /app/actions /app/data

# Copy entrypoint script
COPY docker-entrypoint.sh /app/
RUN chmod +x /app/docker-entrypoint.sh && chown agent:agent /app/docker-entrypoint.sh

USER agent

# Environment
ENV COGNIARK_CONFIG=/app/config
ENV COGNIARK_DATA=/app/data
# Secure logging: suppress SQLx queries to prevent sensitive data exposure
ENV RUST_LOG=info,sqlx::query=warn,sea_orm=warn,hyper=warn,reqwest=warn

# Expose HTTP API port
EXPOSE 17990

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:17990/health || exit 1

# Run with entrypoint script that checks for volume mounts
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["--headless"]
