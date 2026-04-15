@echo off
REM AgentArk CLI wrapper for Windows
REM Usage: agentark chat | pulse | setup | start | stop | logs | status

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
if "%CMD%"=="setup" (
    docker exec -it agentark /app/agentark --setup
    goto :eof
)
if "%CMD%"=="start" (
    docker compose up -d --build
    call :verify_lightpanda || exit /b 1
    echo.
    echo AgentArk is running!
    echo   Web UI: http://localhost:8990
    goto :eof
)
if "%CMD%"=="stop" (
    docker compose down
    goto :eof
)
if "%CMD%"=="restart" (
    docker compose down
    docker compose up -d
    call :verify_lightpanda || exit /b 1
    goto :eof
)
if "%CMD%"=="logs" (
    docker compose logs -f --tail=100
    goto :eof
)
if "%CMD%"=="status" (
    docker compose ps
    goto :eof
)
if "%CMD%"=="update" (
    docker compose up -d --build
    call :verify_lightpanda || exit /b 1
    echo Update complete!
    goto :eof
)

:verify_lightpanda
echo Verifying bundled Lightpanda runtime...
set "LIGHTPANDA_RETRIES=20"
:verify_lightpanda_loop
docker compose exec -T agentark-control sh -lc "command -v lightpanda >/dev/null 2>&1" >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo Lightpanda is available inside the AgentArk runtime.
    exit /b 0
)
set /a LIGHTPANDA_RETRIES-=1
if %LIGHTPANDA_RETRIES% LEQ 0 (
    echo Lightpanda is missing from the bundled AgentArk runtime. Update or rebuild before relying on the free search fallback.
    exit /b 1
)
timeout /t 2 >nul
goto verify_lightpanda_loop

echo AgentArk CLI
echo.
echo Usage: agentark ^<command^>
echo.
echo   chat       Interactive CLI chat with your agent
echo   pulse      Run ArkPulse health check
echo   setup      Run setup wizard
echo   start      Start AgentArk
echo   stop       Stop AgentArk
echo   restart    Restart AgentArk
echo   logs       View live logs
echo   status     Show running containers
echo   update     Rebuild and restart
