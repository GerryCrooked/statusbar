# Admin-only rollback of client binary/config to latest backup made by Install-VGStatusbar.ps1.
[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$principal=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Administrator required'}
$dataDir=Join-Path $env:ProgramData 'VGStatusbar'
$backup=Get-ChildItem -LiteralPath $dataDir -Directory -Filter 'backup-*' -ErrorAction Stop |
  Sort-Object Name -Descending | Where-Object {
    (Test-Path (Join-Path $_.FullName 'vg-status-client.exe')) -and (Test-Path (Join-Path $_.FullName 'client.json'))
  } | Select-Object -First 1
if(-not $backup){throw 'No complete backup found.'}
Get-Process vg-status-client -ErrorAction SilentlyContinue | Stop-Process -Force
Copy-Item -LiteralPath (Join-Path $backup.FullName 'vg-status-client.exe') -Destination (Join-Path $env:ProgramFiles 'VGStatusbar\vg-status-client.exe') -Force
Copy-Item -LiteralPath (Join-Path $backup.FullName 'client.json') -Destination (Join-Path $dataDir 'client.json') -Force
Write-Host ('Restored client from: '+$backup.FullName)
Write-Host 'Restart the client in an interactive Windows session.'
