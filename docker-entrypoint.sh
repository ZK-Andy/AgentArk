#!/bin/bash
# CogniArk Docker Entrypoint
# Checks for proper volume mounts and warns users about data persistence

set -e

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

# Ensure directories exist with proper permissions
ensure_directories() {
    mkdir -p /app/data/actions 2>/dev/null || true
    mkdir -p /app/config 2>/dev/null || true
}

# Check if data volume is mounted (by checking if it's a mount point or has data)
check_volume_mount() {
    # Ensure directories exist
    ensure_directories

    # If the directory is empty and writable, it might not be a volume mount
    if [ -z "$(ls -A /app/data 2>/dev/null)" ]; then
        # Check if this is a fresh volume (has .volume marker) or no volume
        if [ ! -f /app/data/.volume_initialized ]; then
            echo ""
            echo -e "${YELLOW}============================================${NC}"
            echo -e "${YELLOW}  FIRST RUN DETECTED${NC}"
            echo -e "${YELLOW}============================================${NC}"
            echo ""
            echo "Initializing data directory..."
            touch /app/data/.volume_initialized
            echo ""
            echo -e "${GREEN}Your data will be stored in Docker volumes.${NC}"
            echo -e "${GREEN}It will persist across container rebuilds.${NC}"
            echo ""
        fi
    else
        echo -e "${GREEN}Existing data found - your conversations and actions are preserved.${NC}"
    fi
}

# Run check
check_volume_mount

# Print startup banner
echo ""
echo "============================================"
echo "  CogniArk Starting..."
echo "  Web UI: http://localhost:17990"
echo "============================================"
echo ""

# Execute the main application
exec /app/cogniark "$@"
