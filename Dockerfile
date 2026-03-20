# =============================================================================
# AgentArk - AI Agent Docker Image
# =============================================================================
#
# RECOMMENDED: Use docker-compose for automatic data persistence
#
#   ./scripts/start.sh  (Linux/Mac)
#   scripts/start.bat   (Windows)
#   docker-compose up -d --build
#
# Your data (conversations, skills, settings) is automatically preserved
# across rebuilds when using docker-compose.
#
# =============================================================================
# MANUAL DOCKER RUN (must include volumes to preserve data):
#
#   docker run -d -p 8990:8990 \
#     -v agentark-data:/app/data \
#     -v agentark-config:/app/config \
#     --name agentark \
#     agentark:latest
#
# WARNING: Running without -v volumes will LOSE YOUR DATA on container removal!
# =============================================================================

# ── Stage 1: Rust build (with BuildKit cache for fast rebuilds) ─────────────
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Low-memory build: thin LTO + single-job cargo to reduce peak RAM in Docker
# This is slower, but it rebuilds reliably on 2GB-class Docker Desktop setups.
ENV CARGO_BUILD_JOBS=1
ENV CARGO_PROFILE_RELEASE_LTO=thin
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=4

# Build dependencies with cache mount (survives across docker builds)
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release -j 1 && rm -rf src

# Copy source + assets (logo.svg is included at compile time via include_str!)
# CACHEBUST invalidates the layer when source changes aren't detected by Docker
ARG CACHEBUST=0
COPY src ./src
COPY assets ./assets

# Build for release with cache mount, then copy binary out of cache
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    rm -f target/release/agentark target/release/deps/agentark-* && \
    touch src/main.rs && cargo build --release -j 1 && \
    cp target/release/agentark /app/agentark-bin

# ── Stage 2: Frontend build ──────────────────────────────────────────────────
FROM node:20-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm pkg delete devDependencies.@rollup/rollup-win32-x64-msvc 2>/dev/null; npm ci
ARG FRONTEND_CACHEBUST=0
COPY frontend/src ./src
COPY frontend/index.html frontend/tsconfig.json frontend/tsconfig.node.json frontend/vite.config.ts ./
RUN npm run build

# ── Stage 3: Node.js bridges build ───────────────────────────────────────────
# Build node_modules here (git available), then copy only the result to runtime
FROM node:20-slim AS node-builder

RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /bridge/whatsapp-bridge
COPY services/whatsapp-bridge/package.json services/whatsapp-bridge/package-lock.json ./
RUN printf '[url "https://github.com/"]\n\tinsteadOf = ssh://git@github.com/\n\tinsteadOf = git@github.com:\n' > /root/.gitconfig && \
    npm ci --omit=dev && \
    npm cache clean --force && \
    rm -rf /root/.npm /root/.gitconfig /tmp/*
COPY services/whatsapp-bridge/index.js ./

# Playwright bridge (skip browser download; runtime image provides browsers)
WORKDIR /bridge/playwright-bridge
ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
COPY services/playwright-bridge/package.json services/playwright-bridge/package-lock.json ./
RUN npm ci --omit=dev && \
    npm cache clean --force && \
    rm -rf /root/.npm /tmp/*
COPY services/playwright-bridge/index.js ./

# Remotion video template (pre-install node_modules for fast renders)
WORKDIR /bridge/remotion-template
COPY services/remotion-template/package.json services/remotion-template/package-lock.json ./
RUN npm ci --omit=dev 2>/dev/null && \
    npm cache clean --force && \
    rm -rf /root/.npm /tmp/*
COPY services/remotion-template/src ./src
COPY services/remotion-template/tsconfig.json services/remotion-template/remotion.config.ts ./

# ── Stage 4: Minimal runtime ────────────────────────────────────────────────
FROM mcr.microsoft.com/playwright:v1.58.2-noble

RUN apt-get update && apt-get upgrade -y && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gosu \
    ffmpeg \
    git \
    python3 \
    python3-venv \
    zstd \
    && mkdir -p --mode=0755 /usr/share/keyrings \
    && curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/noble.noarmor.gpg \
        -o /usr/share/keyrings/tailscale-archive-keyring.gpg \
    && curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/noble.tailscale-keyring.list \
        -o /etc/apt/sources.list.d/tailscale.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends tailscale \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create non-root user + all directories in one layer
RUN useradd --create-home --shell /usr/sbin/nologin agent && \
    mkdir -p /app/data /app/data/skills /app/data/whatsapp-auth /app/data/tailscale /app/config /app/whatsapp-bridge /app/playwright-bridge /app/mem0-bridge && \
    chown -R agent:agent /app

ENV PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    MEM0_VENV=/opt/mem0-venv \
    MEM0_PYTHON=/opt/mem0-venv/bin/python

# Install Mem0 Python dependencies in an isolated virtualenv.
COPY services/mem0-bridge/requirements.txt /app/mem0-bridge/
RUN python3 -m venv "${MEM0_VENV}" && \
    "${MEM0_VENV}/bin/pip" install --upgrade pip setuptools wheel && \
    "${MEM0_VENV}/bin/pip" install -r /app/mem0-bridge/requirements.txt

# Copy Mem0 bridge app
COPY --chown=agent:agent services/mem0-bridge/app.py /app/mem0-bridge/


# Download cloudflared for built-in tunnel support (zero-friction remote access)
# Pinned version for reproducible builds — update deliberately after testing.
ARG CLOUDFLARED_VERSION=2026.2.0
RUN curl -fsSL --retry 3 \
    "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-linux-amd64" \
    -o /usr/local/bin/cloudflared && \
    chmod +x /usr/local/bin/cloudflared

# Download Lightpanda for fast headless content extraction (~6MB vs ~1.5GB Chromium)
# Used as fast-path for http_get, web search scraping, and research content fetching.
# Playwright remains for screenshots and complex SPA interaction.
ARG LIGHTPANDA_RELEASE=nightly
RUN curl -fsSL --retry 3 \
    "https://github.com/lightpanda-io/browser/releases/download/${LIGHTPANDA_RELEASE}/lightpanda-x86_64-linux" \
    -o /usr/local/bin/lightpanda && \
    chmod +x /usr/local/bin/lightpanda

# Install the Ollama CLI so AgentArk can expose `ollama launch` application registry actions.
ARG OLLAMA_LINUX_URL=https://ollama.com/download/ollama-linux-amd64.tar.zst
RUN curl -fsSL --retry 3 "${OLLAMA_LINUX_URL}" | tar --zstd -x -C /usr

RUN apt-get purge -y --auto-remove curl && rm -rf /var/lib/apt/lists/*

# Copy pre-built bridges with node_modules (owned by agent)
COPY --from=node-builder --chown=agent:agent /bridge/whatsapp-bridge /app/whatsapp-bridge
COPY --from=node-builder --chown=agent:agent /bridge/playwright-bridge /app/playwright-bridge

# Copy Remotion template with pre-installed node_modules (for video generation)
COPY --from=node-builder --chown=agent:agent /bridge/remotion-template /app/services/remotion-template

# Copy AgentArk binary from builder
COPY --from=builder --chown=agent:agent /app/agentark-bin /app/agentark

# Copy assets directly from build context (not part of Rust compilation)
COPY --chown=agent:agent config /app/config
COPY --chown=agent:agent skills /app/skills
COPY --chown=agent:agent assets /app/assets
# Copy frontend assets (built in Docker, not from host)
COPY --from=frontend-builder --chown=agent:agent /app/frontend/dist /app/frontend/dist
# frontend/legacy is optional (static fallback assets)
RUN mkdir -p /app/frontend/legacy && chown agent:agent /app/frontend/legacy

# Copy entrypoint script (fix Windows CRLF line endings)
COPY --chown=agent:agent docker-entrypoint.sh /app/
RUN sed -i 's/\r$//' /app/docker-entrypoint.sh && chmod +x /app/docker-entrypoint.sh

# Start as root — entrypoint will fix docker socket perms then drop to agent

# Environment
ENV AGENTARK_CONFIG=/app/config
ENV AGENTARK_DATA=/app/data
ENV TS_STATE_DIR=/app/data/tailscale
ENV TS_SOCKET=/app/data/tailscale/tailscaled.sock
ENV TS_USERSPACE=true
# Playwright browsers are preinstalled in the base image
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
# Default bridge URL for in-container Playwright service
ENV PLAYWRIGHT_BRIDGE_URL=http://127.0.0.1:3100
# Secure logging: suppress SQLx queries to prevent sensitive data exposure
ENV RUST_LOG=info,sqlx::query=warn,sea_orm=warn,hyper=warn,reqwest=warn

# Expose HTTP API port
EXPOSE 8990

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8990/health', timeout=5)" || exit 1

# Run with entrypoint script that checks for volume mounts
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["--headless"]
