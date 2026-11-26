# Installs the client for all users of this computer. Run elevated (or as SYSTEM from your deployment tool).
#   .\install-client.ps1 -ExePath .\notiflag-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
# -Name must match NOTIFLAG_APP_NAME if you built a rebranded client.
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$ExePath,
  [Parameter(Mandatory)][string]$ServerUrl,
  [Parameter(Mandatory)][string]$Token,
  [string]$Name = 'Notiflag'
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run as administrator.' }
if ($ServerUrl -notmatch '^https://') { throw 'ServerUrl must start with https://' }

$programDir = Join-Path $env:ProgramFiles $Name
$configDir  = Join-Path $env:ProgramData $Name
$target     = Join-Path $programDir 'notiflag-client.exe'
New-Item -ItemType Directory -Force -Path $programDir, $configDir | Out-Null

# Replace a running copy: stop it, copy, and let the next sign-in (or the line below) start it again.
Get-Process -Name 'notiflag-client' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Copy-Item (Resolve-Path $ExePath).Path $target -Force

# A script installation from before the rename to Notiflag: stop it and remove it. Its configuration is
# replaced by the one written below. (An MSI installation of Statusbar is left alone: remove it with
# msiexec, or simply install Notiflag.msi, which replaces it.)
if ($Name -eq 'Notiflag' -and -not (Test-Path 'HKLM:\Software\Statusbar')) {
  Get-Process -Name 'statusbar-client' -ErrorAction SilentlyContinue | Stop-Process -Force
  Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'Statusbar' -ErrorAction SilentlyContinue
  Remove-Item -Recurse -Force (Join-Path $env:ProgramFiles 'Statusbar'), (Join-Path $env:ProgramData 'Statusbar') -ErrorAction SilentlyContinue
}

$json = @{ server_url = $ServerUrl.TrimEnd('/'); client_token = $Token } | ConvertTo-Json
[IO.File]::WriteAllText((Join-Path $configDir 'client.json'), $json, (New-Object Text.UTF8Encoding($false)))
# Users may read the configuration, only administrators and SYSTEM may change it.
& icacls.exe $configDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null

New-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $Name -PropertyType String -Value ('"' + $target + '"') -Force | Out-Null
Write-Host "Installed to $programDir. The client starts at the next sign-in of each user."
