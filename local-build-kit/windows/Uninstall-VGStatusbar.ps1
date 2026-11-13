[CmdletBinding()]
param([switch]$KeepConfiguration)
$ErrorActionPreference='Stop'
$principal=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Administrator required'}
Get-Process -Name vg-status-client -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name VGStatusbar -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $env:ProgramFiles 'VGStatusbar') -Recurse -Force -ErrorAction SilentlyContinue
if(-not $KeepConfiguration){Remove-Item -LiteralPath (Join-Path $env:ProgramData 'VGStatusbar') -Recurse -Force -ErrorAction SilentlyContinue}
Write-Host 'VGStatusbar uninstalled.'
