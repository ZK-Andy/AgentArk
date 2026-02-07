@echo off
REM CogniArk Easy Start Script for Windows
REM
REM This script ensures your data is always preserved across updates.
REM Just run: start.bat
REM
REM Commands:
REM   start.bat          - Start CogniArk
REM   start.bat stop     - Stop CogniArk
REM   start.bat restart  - Restart CogniArk
REM   start.bat logs     - View logs
REM   start.bat update   - Rebuild and restart (preserves data)

setlocal

if "%1"=="" goto start
if "%1"=="start" goto start
if "%1"=="stop" goto stop
if "%1"=="restart" goto restart
if "%1"=="logs" goto logs
if "%1"=="update" goto update
goto usage

:start
echo Starting CogniArk...
docker compose up -d --build
echo.
echo CogniArk is running at http://localhost:17990
echo Your data is safely stored in Docker volumes (cogniark-data, cogniark-config)
goto end

:stop
echo Stopping CogniArk...
docker compose down
echo CogniArk stopped. Your data is preserved.
goto end

:restart
echo Restarting CogniArk...
docker compose restart
goto end

:logs
docker compose logs -f
goto end

:update
echo Updating CogniArk (your data will be preserved)...
docker compose down
docker compose build --no-cache
docker compose up -d
echo Update complete! Your data is intact.
goto end

:usage
echo Usage: start.bat [start^|stop^|restart^|logs^|update]
goto end

:end
endlocal
