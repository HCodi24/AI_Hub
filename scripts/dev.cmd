@echo off
rem Dev launcher: builds web (npm) then Rust (cargo) via dev.ps1, then runs the dev binary.
rem %~dp0 keeps the script path correct with spaces; %* forwards all application arguments.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev.ps1" %*
exit /b %errorlevel%
