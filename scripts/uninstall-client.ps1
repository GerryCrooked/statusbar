# Run as administrator; sign out first to stop user-session clients.
$ErrorActionPreference='Stop'
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'VGStatusbar' -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:ProgramFiles 'VGStatusbar') -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:ProgramData 'VGStatusbar') -Recurse -Force -ErrorAction SilentlyContinue
Write-Host 'Removed files and autostart; sign out to terminate running clients.'
