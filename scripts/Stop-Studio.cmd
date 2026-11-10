@echo off
cd /d "%~dp0"
powershell -NoProfile -Command "$expected=Join-Path $PWD 'Xformed-Studio.exe'; Get-Process -Name Xformed-Studio -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $expected } | Stop-Process"
