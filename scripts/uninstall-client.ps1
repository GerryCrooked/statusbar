# Removes the client from this computer. Run elevated.
[CmdletBinding()]
param([string]$Name = 'Notiflag')
$ErrorActionPreference = 'SilentlyContinue'
# Also removes a script installation from before the rename to Notiflag (Statusbar); an MSI installation
# (HKLM:\Software\Statusbar) is left to msiexec.
$names = @($Name) + $(if ($Name -eq 'Notiflag' -and -not (Test-Path 'HKLM:\Software\Statusbar')) { 'Statusbar' })
Get-Process -Name 'notiflag-client', 'statusbar-client' | Stop-Process -Force
Start-Sleep -Milliseconds 500
foreach ($n in $names) {
  Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $n
  Remove-Item -Recurse -Force (Join-Path $env:ProgramFiles $n), (Join-Path $env:ProgramData $n)
}
Write-Host 'Removed.'
