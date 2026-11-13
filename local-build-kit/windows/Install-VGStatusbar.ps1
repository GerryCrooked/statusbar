# Run elevated on a domain Windows client, from extracted PRIVATE deployment package.
[CmdletBinding()]
param([switch]$NoAutostart)
$ErrorActionPreference='Stop'
$principal=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Administrator required'}
$srcExe=Join-Path $PSScriptRoot 'vg-status-client.exe'
$srcCfg=Join-Path $PSScriptRoot 'client.json'
if(-not (Test-Path $srcExe) -or -not (Test-Path $srcCfg)){throw 'Missing EXE or client.json'}
$sig=Get-AuthenticodeSignature -LiteralPath $srcExe
if($sig.Status -ne 'Valid'){throw ('Invalid or untrusted Authenticode signature: '+$sig.Status)}
$cfg=Get-Content -LiteralPath $srcCfg -Raw | ConvertFrom-Json
if($cfg.server_url -notmatch '^https://' -or [string]::IsNullOrWhiteSpace($cfg.client_token)){throw 'Invalid client.json'}
$programDir=Join-Path $env:ProgramFiles 'VGStatusbar'
$dataDir=Join-Path $env:ProgramData 'VGStatusbar'
$backupDir=Join-Path $dataDir ('backup-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force -Path $programDir,$dataDir,$backupDir | Out-Null
# Keep backup admin/SYSTEM only.
& icacls.exe $backupDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' | Out-Null
if($LASTEXITCODE -ne 0){throw 'Failed to ACL backup directory'}
$destExe=Join-Path $programDir 'vg-status-client.exe'
$destCfg=Join-Path $dataDir 'client.json'
if(Test-Path $destExe){Copy-Item $destExe $backupDir -Force}
if(Test-Path $destCfg){Copy-Item $destCfg $backupDir -Force}
Get-Process -Name 'vg-status-client' -ErrorAction SilentlyContinue | Stop-Process -Force
Copy-Item -LiteralPath $srcExe -Destination $destExe -Force
# Use an atomic replacement for the config where possible.
$tmpCfg=Join-Path $dataDir 'client.json.new'
[System.IO.File]::WriteAllText($tmpCfg,(Get-Content -LiteralPath $srcCfg -Raw),(New-Object System.Text.UTF8Encoding($false)))
Move-Item -LiteralPath $tmpCfg -Destination $destCfg -Force
& icacls.exe $programDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null
if($LASTEXITCODE -ne 0){throw 'Failed to ACL program directory'}
& icacls.exe $dataDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null
if($LASTEXITCODE -ne 0){throw 'Failed to ACL data directory'}
& icacls.exe $destCfg /inheritance:r /grant:r '*S-1-5-18:F' '*S-1-5-32-544:F' '*S-1-5-32-545:R' | Out-Null
if($LASTEXITCODE -ne 0){throw 'Failed to ACL client config'}
$reg='HKLM:\Software\Microsoft\Windows\CurrentVersion\Run'
if(-not $NoAutostart){New-ItemProperty -Path $reg -Name VGStatusbar -PropertyType String -Value ('"'+$destExe+'"') -Force | Out-Null}
Write-Host ('Installed: '+$destExe)
Write-Host ('Backup: '+$backupDir)
if($NoAutostart){Write-Host 'Pilot mode: no autostart. Launch EXE manually in user session.'}
else{Write-Host 'Autostart configured at next interactive sign-in.'}
Write-Warning 'Current server uses a shared token readable by local Users. Never deploy the package publicly.'
