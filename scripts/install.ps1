# AgentArk Installer for Windows
#
# Usage: irm https://raw.githubusercontent.com/agentark-ai/AgentArk/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$InstallDir = Join-Path $env:USERPROFILE "agentark"
$SourceDir = Join-Path $InstallDir "source"
$ReleaseRepo = if ([string]::IsNullOrWhiteSpace($env:AGENTARK_RELEASE_REPO)) { "agentark-ai/AgentArk" } else { $env:AGENTARK_RELEASE_REPO.Trim() }
$RepoUrl = "https://github.com/$ReleaseRepo.git"
$ImageRepository = if ([string]::IsNullOrWhiteSpace($env:AGENTARK_IMAGE_REPOSITORY)) { "ghcr.io/agentark-ai/agentark" } else { $env:AGENTARK_IMAGE_REPOSITORY.Trim() }

function Get-AgentArkLatestReleaseTag {
    $refs = & docker run --rm alpine/git ls-remote --tags --refs $RepoUrl "v*" 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($refs)) {
        return $null
    }

    $tags = $refs |
        ForEach-Object {
            $parts = ($_ -split '\s+')
            if ($parts.Length -gt 1) { $parts[-1] -replace '^refs/tags/', '' } else { $null }
        } |
        Where-Object { $_ -match '^v\d+\.\d+\.\d+$' }

    if (-not $tags) {
        return $null
    }

    return $tags |
        Sort-Object { [version]($_.Substring(1)) } |
        Select-Object -Last 1
}

function Get-AgentArkReleaseVersionFromTag {
    param([string]$Tag)
    if ([string]::IsNullOrWhiteSpace($Tag)) {
        return ""
    }
    return $Tag.TrimStart("v", "V")
}

function Ensure-AgentArkEnvFile {
    $envPath = Join-Path $SourceDir ".env"
    if (-not (Test-Path $envPath) -and (Test-Path (Join-Path $SourceDir ".env.example"))) {
        Copy-Item (Join-Path $SourceDir ".env.example") $envPath
    }
    if (-not (Test-Path $envPath)) {
        New-Item -ItemType File -Path $envPath -Force | Out-Null
    }
    return $envPath
}

function Set-AgentArkEnvValue {
    param(
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $envPath = Ensure-AgentArkEnvFile
    $lines = if (Test-Path $envPath) { [System.Collections.Generic.List[string]](Get-Content $envPath) } else { [System.Collections.Generic.List[string]]::new() }
    $updated = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -like "$Key=*") {
            $lines[$i] = "$Key=$Value"
            $updated = $true
        }
    }
    if (-not $updated) {
        $lines.Add("$Key=$Value")
    }
    Set-Content -Path $envPath -Value $lines -Encoding ASCII
}

function Set-AgentArkPinnedRelease {
    param([Parameter(Mandatory = $true)][string]$Tag)

    $version = Get-AgentArkReleaseVersionFromTag $Tag
    Set-AgentArkEnvValue -Key "AGENTARK_IMAGE" -Value "${ImageRepository}:$version"
    Set-AgentArkEnvValue -Key "AGENTARK_RELEASE_REPO" -Value $ReleaseRepo
    Set-AgentArkEnvValue -Key "AGENTARK_RELEASE_TAG" -Value $Tag
}

function Assert-AgentArkCleanCheckout {
    $status = & docker run --rm -v "${InstallDir}:/work" -w /work alpine/git git -C /work/source status --porcelain --untracked-files=no 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect the AgentArk source checkout."
    }
    if (-not [string]::IsNullOrWhiteSpace(($status | Out-String))) {
        throw "Tracked local changes were found in $SourceDir. Resolve them before reinstalling."
    }
}

function Write-AgentArkPortWarning {
    param(
        [int]$Port,
        [string]$ServiceName
    )

    try {
        $listeners = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction Stop
    } catch {
        $listeners = @()
    }

    if ($listeners.Count -gt 0) {
        Write-Host "Warning: TCP port $Port is already in use. $ServiceName may fail to start unless you stop the existing listener or override the port." -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "=========================================" -ForegroundColor White
Write-Host "  AgentArk Installer" -ForegroundColor White
Write-Host "  Think. Act. Remember. Securely." -ForegroundColor White
Write-Host "=========================================" -ForegroundColor White
Write-Host ""

$docker = Get-Command docker -ErrorAction SilentlyContinue
if (-not $docker) {
    Write-Host "Docker not found." -ForegroundColor Red
    Write-Host "Please install Docker Desktop: https://docs.docker.com/desktop/install/windows-install/" -ForegroundColor Cyan
    exit 1
}
Write-Host "[1/4] Docker found." -ForegroundColor Green

$composeCheck = docker compose version 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Docker Compose not found. Please install Docker Desktop." -ForegroundColor Red
    exit 1
}
Write-Host "[2/4] Docker Compose found." -ForegroundColor Green

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$TargetReleaseTag = if ([string]::IsNullOrWhiteSpace($env:AGENTARK_RELEASE_TAG)) { Get-AgentArkLatestReleaseTag } else { $env:AGENTARK_RELEASE_TAG.Trim() }
if ([string]::IsNullOrWhiteSpace($TargetReleaseTag)) {
    throw "Unable to resolve the latest tagged AgentArk release."
}

if (-not (Test-Path (Join-Path $SourceDir ".git"))) {
    Write-Host "Cloning AgentArk $TargetReleaseTag into $SourceDir..." -ForegroundColor Cyan
    & docker run --rm -v "${InstallDir}:/work" -w /work alpine/git clone --branch $TargetReleaseTag --depth 1 $RepoUrl source
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to clone the AgentArk release checkout."
    }
} else {
    Write-Host "Existing source checkout found at $SourceDir" -ForegroundColor Green
    Assert-AgentArkCleanCheckout
    & docker run --rm -v "${InstallDir}:/work" -w /work alpine/git git -C /work/source fetch --tags --force origin
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to fetch AgentArk release tags."
    }
    & docker run --rm -v "${InstallDir}:/work" -w /work alpine/git git -C /work/source checkout --force $TargetReleaseTag
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to switch the AgentArk checkout to $TargetReleaseTag."
    }
}

if (-not (Test-Path (Join-Path $SourceDir "docker-compose.yml"))) {
    throw "Missing $SourceDir\docker-compose.yml after checkout."
}

Set-AgentArkPinnedRelease -Tag $TargetReleaseTag
Write-Host "[3/4] Source checkout ready at $SourceDir" -ForegroundColor Green

$cmdWrapper = @'
@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0source\scripts\agentark-release-cli.ps1" %*
'@
Set-Content -Path (Join-Path $InstallDir "agentark.cmd") -Value $cmdWrapper -Encoding ASCII

$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$userPath;$InstallDir", "User")
    $env:PATH = "$env:PATH;$InstallDir"
    Write-Host "Added $InstallDir to your PATH." -ForegroundColor Green
}

Write-Host "Downloading AgentArk container image for $TargetReleaseTag..." -ForegroundColor Cyan
$postgresPort = 5432
if ($env:AGENTARK_POSTGRES_PORT -match '^\d+$') {
    $postgresPort = [int]$env:AGENTARK_POSTGRES_PORT
}
Write-AgentArkPortWarning -Port $postgresPort -ServiceName "Postgres"
Write-AgentArkPortWarning -Port 8990 -ServiceName "AgentArk Web UI"

Push-Location $SourceDir
try {
    Write-Host "[4/4] Starting AgentArk..." -ForegroundColor Green
    & docker compose pull
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to pull AgentArk images."
    }
    & docker compose up -d
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start AgentArk."
    }

    $lightpandaReady = $false
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        & docker compose exec -T agentark-control sh -lc "command -v lightpanda >/dev/null 2>&1" *> $null
        if ($LASTEXITCODE -eq 0) {
            $lightpandaReady = $true
            break
        }
        Start-Sleep -Seconds 2
    }
    if (-not $lightpandaReady) {
        throw "Lightpanda is missing from the bundled AgentArk runtime. Update or rebuild before relying on the free search fallback."
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "=========================================" -ForegroundColor White
Write-Host "  AgentArk is running!" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor White
Write-Host ""
Write-Host "  Web UI:  http://localhost:8990" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Commands (run from anywhere):" -ForegroundColor White
Write-Host "    agentark chat       Interactive CLI chat"
Write-Host "    agentark pulse      Run ArkPulse health check"
Write-Host "    agentark stop       Stop AgentArk"
Write-Host "    agentark update     Install the latest tagged release and restart"
Write-Host "    agentark logs       View logs"
Write-Host "    agentark status     Show status"
Write-Host ""
Write-Host "  App data is stored in Docker volumes and survives updates." -ForegroundColor Yellow
Write-Host "  Postgres has its own volume; use 'docker compose down -v' to reset everything." -ForegroundColor Yellow
Write-Host ""
