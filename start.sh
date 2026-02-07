#!/bin/bash
# CogniArk Easy Start Script
#
# This script ensures your data is always preserved across updates.
# Just run: ./start.sh
#
# Commands:
#   ./start.sh          - Start CogniArk
#   ./start.sh stop     - Stop CogniArk
#   ./start.sh restart  - Restart CogniArk
#   ./start.sh logs     - View logs
#   ./start.sh update   - Rebuild and restart (preserves data)
#   ./start.sh backup   - Backup your data

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

case "${1:-start}" in
    start)
        echo -e "${GREEN}Starting CogniArk...${NC}"
        docker compose up -d --build
        echo -e "${GREEN}CogniArk is running at http://localhost:17990${NC}"
        echo -e "${YELLOW}Your data is safely stored in Docker volumes (cogniark-data, cogniark-config)${NC}"
        ;;
    stop)
        echo -e "${YELLOW}Stopping CogniArk...${NC}"
        docker compose down
        echo -e "${GREEN}CogniArk stopped. Your data is preserved.${NC}"
        ;;
    restart)
        echo -e "${YELLOW}Restarting CogniArk...${NC}"
        docker compose restart
        ;;
    logs)
        docker compose logs -f
        ;;
    update)
        echo -e "${YELLOW}Updating CogniArk (your data will be preserved)...${NC}"
        docker compose down
        docker compose build --no-cache
        docker compose up -d
        echo -e "${GREEN}Update complete! Your data is intact.${NC}"
        ;;
    backup)
        BACKUP_DIR="./backups/$(date +%Y%m%d_%H%M%S)"
        mkdir -p "$BACKUP_DIR"
        echo -e "${YELLOW}Backing up data to $BACKUP_DIR...${NC}"
        docker run --rm -v cogniark-data:/data -v "$(pwd)/$BACKUP_DIR":/backup alpine tar czf /backup/cogniark-data.tar.gz -C /data .
        docker run --rm -v cogniark-config:/data -v "$(pwd)/$BACKUP_DIR":/backup alpine tar czf /backup/cogniark-config.tar.gz -C /data .
        echo -e "${GREEN}Backup complete!${NC}"
        ;;
    *)
        echo "Usage: ./start.sh [start|stop|restart|logs|update|backup]"
        exit 1
        ;;
esac
