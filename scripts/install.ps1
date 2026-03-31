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
  postgres:
    image: postgres:16-alpine
    container_name: agentark-postgres
    restart: unless-stopped
    security_opt:
      - no-new-privileges:true
    environment:
      - POSTGRES_DB=${AGENTARK_POSTGRES_DB:-agentark}
      - POSTGRES_USER=${AGENTARK_POSTGRES_USER:-agentark}
      - POSTGRES_PASSWORD=${AGENTARK_POSTGRES_PASSWORD:-agentark}
    ports:
      - "127.0.0.1:${AGENTARK_POSTGRES_PORT:-5432}:5432"
    volumes:
      - agentark-postgres-data:/var/lib/postgresql/data
    networks:
      - agent-network
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${AGENTARK_POSTGRES_USER:-agentark} -d ${AGENTARK_POSTGRES_DB:-agentark}"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 10s
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 1G
        reservations:
          cpus: '0.25'
          memory: 256M

  agentark:
    image: ghcr.io/agentark-ai/agentark:latest
    container_name: agentark
    restart: unless-stopped
    ports:
      - "127.0.0.1:8990:8990"
    volumes:
      - agentark-data:/app/data
      - agentark-config:/app/config
    depends_on:
      postgres:
        condition: service_healthy
      docker-socket-proxy:
        condition: service_started
    environment:
      - RUST_LOG=info,sqlx::query=warn,sea_orm=warn,hyper=warn,reqwest=warn
      - AGENTARK_CONFIG=/app/config
      - AGENTARK_DATA=/app/data
      - AGENTARK_DATABASE_URL=postgres://${AGENTARK_POSTGRES_USER:-agentark}:${AGENTARK_POSTGRES_PASSWORD:-agentark}@postgres:5432/${AGENTARK_POSTGRES_DB:-agentark}
      - AGENTARK_DB_MAX_CONNECTIONS=${AGENTARK_DB_MAX_CONNECTIONS:-20}
      - AGENTARK_DB_CONNECT_TIMEOUT_SECS=${AGENTARK_DB_CONNECT_TIMEOUT_SECS:-5}
      - AGENTARK_DB_STATEMENT_TIMEOUT_MS=${AGENTARK_DB_STATEMENT_TIMEOUT_MS:-30000}
      - AGENTARK_DB_IDLE_TIMEOUT_SECS=${AGENTARK_DB_IDLE_TIMEOUT_SECS:-300}
      - AGENTARK_DB_SCHEMA=${AGENTARK_DB_SCHEMA:-}
      - AGENTARK_BIND=0.0.0.0:8990
      - DOCKER_HOST=tcp://docker-socket-proxy:2375
    networks:
      - agent-network
    healthcheck:
      test: ["CMD", "python3", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8990/health', timeout=5)"]
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
    image: tecnativa/docker-socket-proxy:0.4.2
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
  agentark-postgres-data:
    name: agentark-postgres-data

networks:
  agent-network:
    driver: bridge
'@

Set-Content -Path "$InstallDir\docker-compose.yml" -Value $composeContent -Encoding UTF8
Write-Host "[3/4] Configuration created at $InstallDir" -ForegroundColor Green

# ── Step 4: Create agentark.cmd CLI wrapper ───────────────────────────────────

$cliContent = @'
@echo off
REM AgentArk CLI — simple commands for your AI agent
REM Usage: agentark chat | pulse | start | stop | logs | status | update

set "CMD=%~1"
if "%CMD%"=="" set "CMD=help"

if "%CMD%"=="chat" (
    docker exec -it agentark /app/agentark --chat
    goto :eof
)
if "%CMD%"=="pulse" (
    docker exec agentark /app/agentark --pulse
    goto :eof
)
if "%CMD%"=="start" (
    docker compose -f "%~dp0docker-compose.yml" up -d
    echo.
    echo AgentArk is running!
    echo   Web UI: http://localhost:8990
    goto :eof
)
if "%CMD%"=="stop" (
    docker compose -f "%~dp0docker-compose.yml" down
    echo Stopped. Your data is preserved.
    goto :eof
)
if "%CMD%"=="restart" (
    docker compose -f "%~dp0docker-compose.yml" down
    docker compose -f "%~dp0docker-compose.yml" up -d
    goto :eof
)
if "%CMD%"=="logs" (
    docker compose -f "%~dp0docker-compose.yml" logs -f --tail=100
    goto :eof
)
if "%CMD%"=="status" (
    docker compose -f "%~dp0docker-compose.yml" ps
    goto :eof
)
if "%CMD%"=="update" (
    docker compose -f "%~dp0docker-compose.yml" pull agentark
    docker compose -f "%~dp0docker-compose.yml" up -d agentark
    echo Update complete!
    goto :eof
)
if "%CMD%"=="setup" (
    docker exec -it agentark /app/agentark --setup
    goto :eof
)

echo AgentArk CLI
echo.
echo Usage: agentark ^<command^>
echo.
echo   chat       Interactive CLI chat with your agent
echo   pulse      Run ArkPulse health check
echo   start      Start AgentArk
echo   stop       Stop AgentArk
echo   restart    Restart AgentArk
echo   logs       View live logs
echo   status     Show running containers
echo   update     Pull latest image and restart
echo   setup      Run setup wizard
'@

Set-Content -Path "$InstallDir\agentark.cmd" -Value $cliContent -Encoding ASCII

# Add install dir to user PATH so 'agentark' works from anywhere
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$userPath;$InstallDir", "User")
    $env:PATH = "$env:PATH;$InstallDir"
    Write-Host "Added $InstallDir to your PATH." -ForegroundColor Green
    Write-Host "  (Open a new terminal if 'agentark' isn't recognized immediately)" -ForegroundColor Yellow
}

Write-Host "[3/4] CLI installed." -ForegroundColor Green

# ── Step 5: Pull image and start ────────────────────────────────────────────

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
Write-Host "  Commands (run from anywhere):" -ForegroundColor White
Write-Host "    agentark chat       Interactive CLI chat"
Write-Host "    agentark pulse      Run ArkPulse health check"
Write-Host "    agentark stop       Stop AgentArk"
Write-Host "    agentark update     Pull latest and restart"
Write-Host "    agentark logs       View logs"
Write-Host "    agentark status     Show status"
Write-Host ""
Write-Host "  App data is stored in Docker volumes and survives updates." -ForegroundColor Yellow
Write-Host "  Postgres has its own volume; use 'docker compose down -v' to reset everything." -ForegroundColor Yellow
Write-Host ""
