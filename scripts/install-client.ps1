# Run as administrator from the extracted project directory, after building client.exe.
# Usage: .\scripts\install-client.ps1 -ExePath .\client\target\x86_64-pc-windows-msvc\release\vg-status-client.exe -ServerUrl https://status.example.local -Token '<CLIENT_TOKEN>'
[CmdletBinding()]
param(
 [Parameter(Mandatory)][string]$ExePath,
 [Parameter(Mandatory)][string]$ServerUrl,
 [Parameter(Mandatory)][string]$Token,
 [int]$PollSeconds=10
)
$ErrorActionPreference='Stop'
$principal=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'PowerShell as administrator required.'}
if($ServerUrl -notmatch '^https://'){throw 'HTTPS ServerUrl required (except manual localhost test).'}
$exe=(Resolve-Path $ExePath).Path
$dir=Join-Path $env:ProgramFiles 'VGStatusbar'
$configDir=Join-Path $env:ProgramData 'VGStatusbar'
New-Item -ItemType Directory -Force -Path $dir,$configDir | Out-Null
Copy-Item $exe (Join-Path $dir 'vg-status-client.exe') -Force
$config=@{server_url=$ServerUrl;client_token=$Token;poll_seconds=$PollSeconds}|ConvertTo-Json
[System.IO.File]::WriteAllText((Join-Path $configDir 'client.json'),$config,(New-Object System.Text.UTF8Encoding($false)))
# Config read access is needed in user session; only Administrators/SYSTEM may write.
& icacls.exe $configDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null
& icacls.exe (Join-Path $configDir 'client.json') /inheritance:r /grant:r '*S-1-5-18:F' '*S-1-5-32-544:F' '*S-1-5-32-545:R' | Out-Null
$reg='HKLM:\Software\Microsoft\Windows\CurrentVersion\Run'
New-ItemProperty -Path $reg -Name 'VGStatusbar' -PropertyType String -Value ('"'+(Join-Path $dir 'vg-status-client.exe')+'"') -Force | Out-Null
Write-Host 'Installed. Sign out and sign in to start client for this user.'
