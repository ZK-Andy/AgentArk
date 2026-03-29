#!/bin/bash
# AgentArk Docker Entrypoint
# - Fixes Docker socket permissions for sandboxed code execution
# - Drops privileges to 'agent' user before starting the app

set -e

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

# Ensure directories exist with proper permissions
ensure_directories() {
    mkdir -p /app/data/skills 2>/dev/null || true
    mkdir -p /app/data/tailscale 2>/dev/null || true
    mkdir -p /app/config 2>/dev/null || true
    chown -R agent:agent /app/data /app/config 2>/dev/null || true
}

# Fix Docker socket permissions so 'agent' can spawn sandboxed containers
setup_docker_socket() {
    if [ -S /var/run/docker.sock ]; then
        # Direct socket mount — fix permissions
        DOCKER_GID=$(stat -c '%g' /var/run/docker.sock)
        if ! getent group "$DOCKER_GID" > /dev/null 2>&1; then
            groupadd -g "$DOCKER_GID" dockerhost 2>/dev/null || true
        fi
        DOCKER_GROUP=$(getent group "$DOCKER_GID" | cut -d: -f1)
        usermod -aG "$DOCKER_GROUP" agent 2>/dev/null || true
        echo -e "${GREEN}Docker socket available — sandboxed code execution enabled${NC}"
    elif [ -n "$DOCKER_HOST" ]; then
        # TCP proxy (docker-socket-proxy) — no socket permissions needed
        echo -e "${GREEN}Docker available via proxy ($DOCKER_HOST) — sandboxed code execution enabled${NC}"
    else
        echo -e "${YELLOW}Docker not available — code execution will use native fallback${NC}"
    fi
}

# Check if data volume is mounted
check_volume_mount() {
    ensure_directories

    if [ -z "$(ls -A /app/data 2>/dev/null)" ]; then
        if [ ! -f /app/data/.volume_initialized ]; then
            echo ""
            echo -e "${YELLOW}============================================${NC}"
            echo -e "${YELLOW}  FIRST RUN DETECTED${NC}"
            echo -e "${YELLOW}============================================${NC}"
            echo ""
            echo "Initializing data directory..."
            touch /app/data/.volume_initialized
            chown agent:agent /app/data/.volume_initialized
            echo ""
            echo -e "${GREEN}Your data will be stored in Docker volumes.${NC}"
            echo -e "${GREEN}It will persist across container rebuilds.${NC}"
            echo ""
        fi
    else
        echo -e "${GREEN}Existing data found - your conversations and skills are preserved.${NC}"
    fi
}

# Run setup as root
setup_docker_socket
check_volume_mount

# WhatsApp bridge is managed by the AgentArk backend (started/stopped via Settings UI)

# Forward Docker secret as env var (if present)
if [ -f /run/secrets/agentark_master_key ]; then
    export AGENTARK_MASTER_PASSWORD=$(cat /run/secrets/agentark_master_key)
    echo -e "${GREEN}Docker secret found — master password will be used for encryption${NC}"
fi

# Print startup banner
echo ""
echo "============================================"
echo "  AgentArk Starting..."
echo "  Web UI: http://localhost:8990"
echo "============================================"
echo ""

start_tailscale_daemon() {
    export TS_STATE_DIR=${TS_STATE_DIR:-/app/data/tailscale}
    export TS_SOCKET=${TS_SOCKET:-/app/data/tailscale/tailscaled.sock}
    export TS_USERSPACE=${TS_USERSPACE:-true}

    if command -v tailscaled >/dev/null 2>&1 && command -v tailscale >/dev/null 2>&1; then
        mkdir -p "$TS_STATE_DIR"
        chown -R agent:agent "$TS_STATE_DIR" 2>/dev/null || true
        rm -f "$TS_SOCKET" 2>/dev/null || true
        echo -e "${GREEN}Starting Tailscale daemon (userspace, persistent state)...${NC}"
        gosu agent tailscaled \
            --statedir="$TS_STATE_DIR" \
            --socket="$TS_SOCKET" \
            --tun=userspace-networking &
        TAILSCALE_PID=$!

        for _ in $(seq 1 20); do
            if [ -S "$TS_SOCKET" ]; then
                echo -e "${GREEN}Tailscale daemon started (PID: $TAILSCALE_PID)${NC}"
                return
            fi
            sleep 1
        done

        echo -e "${YELLOW}Tailscale daemon did not expose its socket in time; Tailscale tunnel actions may fail.${NC}"
    else
        echo -e "${YELLOW}Tailscale runtime not installed; Tailscale tunnel providers will stay unavailable.${NC}"
    fi
}

start_tailscale_daemon

# Start Mem0 memory bridge in background (localhost-only)
start_mem0_bridge() {
    MEM0_PYTHON_BIN=${MEM0_PYTHON:-/opt/mem0-venv/bin/python}
    if [ -x "$MEM0_PYTHON_BIN" ] && [ -f /app/mem0-bridge/app.py ]; then
        echo -e "${GREEN}Starting Mem0 memory bridge (localhost:8991)...${NC}"
        QDRANT_PATH=/app/data/qdrant \
        MODEL_CACHE=/app/data/models \
        gosu agent "$MEM0_PYTHON_BIN" -m uvicorn app:app --host 127.0.0.1 --port 8991 --app-dir /app/mem0-bridge &
        MEM0_PID=$!
        echo -e "${GREEN}Mem0 bridge started (PID: $MEM0_PID)${NC}"
    else
        echo -e "${YELLOW}Mem0 bridge not available (Mem0 Python runtime or bridge files missing)${NC}"
    fi
}

# Start Mem0 bridge in background before main app
start_mem0_bridge

# Start Playwright bridge in background (localhost-only)
start_playwright_bridge() {
    if ! command -v node >/dev/null 2>&1 || [ ! -f /app/playwright-bridge/index.js ] || [ ! -d /app/playwright-bridge/node_modules ]; then
        echo -e "${YELLOW}Playwright bridge not available (Node.js or bridge dependencies missing)${NC}"
        return
    fi

    if [ -n "${PLAYWRIGHT_EXECUTABLE_PATH:-}" ] && [ ! -x "${PLAYWRIGHT_EXECUTABLE_PATH}" ]; then
        if [ -z "$(find "${PLAYWRIGHT_BROWSERS_PATH:-/nonexistent}" -mindepth 1 -maxdepth 1 2>/dev/null | head -n 1)" ]; then
            echo -e "${YELLOW}Playwright bridge not available (no Chromium binary or bundled Playwright browsers found)${NC}"
            return
        fi
    fi

    if command -v node >/dev/null 2>&1 && [ -f /app/playwright-bridge/index.js ]; then
        echo -e "${GREEN}Starting Playwright bridge (localhost:3100)...${NC}"
        PLAYWRIGHT_BROWSERS_PATH=${PLAYWRIGHT_BROWSERS_PATH:-/app/.playwright-browsers} \
        PLAYWRIGHT_EXECUTABLE_PATH=${PLAYWRIGHT_EXECUTABLE_PATH:-} \
        PORT=${PLAYWRIGHT_BRIDGE_PORT:-3100} \
        PLAYWRIGHT_BRIDGE_HOST=${PLAYWRIGHT_BRIDGE_HOST:-127.0.0.1} \
        gosu agent node /app/playwright-bridge/index.js &
        PLAYWRIGHT_PID=$!
        echo -e "${GREEN}Playwright bridge started (PID: $PLAYWRIGHT_PID)${NC}"
    fi
}

start_playwright_bridge

# WhatsApp bridge: started by AgentArk when user enables WhatsApp in Settings UI

# Drop privileges to 'agent' user and exec the app
exec gosu agent /app/agentark "$@"
