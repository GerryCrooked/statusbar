# Removes the client from this computer. Run elevated.
[CmdletBinding()]
param([string]$Name = 'Statusbar')
$ErrorActionPreference = 'SilentlyContinue'
Get-Process -Name 'statusbar-client' | Stop-Process -Force
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $Name
Start-Sleep -Milliseconds 500
Remove-Item -Recurse -Force (Join-Path $env:ProgramFiles $Name), (Join-Path $env:ProgramData $Name)
Write-Host 'Removed.'
