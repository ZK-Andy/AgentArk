# AgentArk Installer for Windows
# Think. Act. Remember. Securely.
#
# Usage: irm https://raw.githubusercontent.com/agentark-ai/AgentArk/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Image = "ghcr.io/agentark-ai/agentark:latest"
$InstallDir = "$env:USERPROFILE\agentark"

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor White
Write-Host "  AgentArk Installer" -ForegroundColor White
Write-Host "  Think. Act. Remember. Securely."
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor White
Write-Host ""

# ── Step 1: Check Docker ────────────────────────────────────────────────────

$docker = Get-Command docker -ErrorAction SilentlyContinue
if (-not $docker) {
    Write-Host "Docker not found." -ForegroundColor Red
    Write-Host "Please install Docker Desktop: https://docs.docker.com/desktop/install/windows-install/" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "After installing, restart this terminal and run the command again." -ForegroundColor Yellow
    exit 1
}
Write-Host "[1/4] Docker found." -ForegroundColor Green

# Verify docker compose
$composeCheck = docker compose version 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Docker Compose not found. Please install Docker Desktop (includes Compose)." -ForegroundColor Red
    exit 1
}
Write-Host "[2/4] Docker Compose found." -ForegroundColor Green

# ── Step 2: Create install directory ────────────────────────────────────────

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# ── Step 3: Generate docker-compose.yml ─────────────────────────────────────

$composeContent = @'
services:
  agentark:
    image: ghcr.io/agentark-ai/agentark:latest
    container_name: agentark
    restart: unless-stopped
    ports:
      - "8990:8990"
    volumes:
      - agentark-data:/app/data
      - agentark-config:/app/config
    depends_on:
      - docker-socket-proxy
    environment:
      - RUST_LOG=info,sqlx::query=warn,sea_orm=warn,hyper=warn,reqwest=warn
      - AGENTARK_CONFIG=/app/config
      - AGENTARK_DATA=/app/data
      - AGENTARK_BIND=0.0.0.0:8990
      - DOCKER_HOST=tcp://docker-socket-proxy:2375
    networks:
      - agent-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8990/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 5s
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G

  docker-socket-proxy:
    image: tecnativa/docker-socket-proxy:latest
    container_name: agentark-docker-proxy
    restart: unless-stopped
    volumes:
      - //var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      - CONTAINERS=1
      - IMAGES=1
      - POST=1
      - EXEC=0
      - VOLUMES=0
      - NETWORKS=0
      - SWARM=0
      - SECRETS=0
      - NODES=0
      - SERVICES=0
      - TASKS=0
      - BUILD=0
      - COMMIT=0
      - CONFIGS=0
      - DISTRIBUTION=0
      - PLUGINS=0
      - SYSTEM=0
    networks:
      - agent-network
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 128M

volumes:
  agentark-data:
    name: agentark-data
  agentark-config:
    name: agentark-config

networks:
  agent-network:
    driver: bridge
'@

Set-Content -Path "$InstallDir\docker-compose.yml" -Value $composeContent -Encoding UTF8
Write-Host "[3/4] Configuration created at $InstallDir" -ForegroundColor Green

# ── Step 4: Pull image and start ────────────────────────────────────────────

Write-Host "Pulling AgentArk image (this may take a minute)..." -ForegroundColor Cyan
Push-Location $InstallDir
try {
    docker compose pull agentark 2>$null

    Write-Host "[4/4] Starting AgentArk..." -ForegroundColor Green
    docker compose up -d
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor White
Write-Host "  AgentArk is running!" -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor White
Write-Host ""
Write-Host "  Web UI:  http://localhost:8990" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Manage:"
Write-Host "    docker compose -f $InstallDir\docker-compose.yml up -d       Start"
Write-Host "    docker compose -f $InstallDir\docker-compose.yml down        Stop"
Write-Host "    docker compose -f $InstallDir\docker-compose.yml pull        Update"
Write-Host "    docker compose -f $InstallDir\docker-compose.yml logs -f     Logs"
Write-Host ""
Write-Host "  Your data is stored in Docker volumes and survives updates." -ForegroundColor Yellow
Write-Host ""
