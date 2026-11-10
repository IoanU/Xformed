@echo off
cd /d "%~dp0"
powershell -NoProfile -Command "$ErrorActionPreference='Stop'; $url='http://127.0.0.1:8080'; $ready=$false; try { $ready=(Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 ($url+'/health')).Content.Trim() -eq 'ok' } catch {}; if(-not $ready) { Start-Process -FilePath (Join-Path $PWD 'Xformed-Studio.exe') -WorkingDirectory $PWD -WindowStyle Hidden }; for($attempt=0;$attempt -lt 50;$attempt++) { try { if((Invoke-WebRequest -UseBasicParsing -TimeoutSec 1 ($url+'/health')).Content.Trim() -eq 'ok') { $ready=$true; break } } catch {}; Start-Sleep -Milliseconds 100 }; if(-not $ready) { throw 'Studio did not start. Check whether port 8080 is in use.' }; Start-Process $url"
if errorlevel 1 pause
