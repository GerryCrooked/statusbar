# Create an internally signed, deployment-ready package. Does not install anything.
[CmdletBinding()]
param(
 [Parameter(Mandatory)][string]$ExePath,
 [Parameter(Mandatory)][string]$ConfigPath,
 [Parameter(Mandatory)][string]$CertificateThumbprint,
 [string]$OutputDirectory = (Join-Path $PSScriptRoot 'release'),
 [string]$TimestampUrl = ''
)
$ErrorActionPreference='Stop'
$exe=(Resolve-Path -LiteralPath $ExePath).Path
$cfg=(Resolve-Path -LiteralPath $ConfigPath).Path
$json=Get-Content -LiteralPath $cfg -Raw | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($json.client_token) -or $json.server_url -notmatch '^https://') {throw 'Invalid config. HTTPS URL and client_token required.'}
$thumb=($CertificateThumbprint -replace '[^0-9a-fA-F]','').ToUpperInvariant()
$cert=Get-Item -LiteralPath "Cert:\CurrentUser\My\$thumb" -ErrorAction Stop
if(-not $cert.HasPrivateKey){throw 'Code signing certificate has no private key'}
if($cert.NotAfter -le (Get-Date)){throw 'Code signing certificate expired'}
$eku=@($cert.EnhancedKeyUsageList | ForEach-Object ObjectId)
if($eku -notcontains '1.3.6.1.5.5.7.3.3'){throw 'Certificate does not have Code Signing EKU'}
$signtool=(Get-Command signtool.exe -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source)
if(-not $signtool){
  $kits='C:\Program Files (x86)\Windows Kits\10\bin'
  $candidate=Get-ChildItem -LiteralPath $kits -Filter signtool.exe -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object {$_.FullName -match '\\x64\\signtool.exe$'} |
    Sort-Object FullName -Descending | Select-Object -First 1
  if($candidate){$signtool=$candidate.FullName}
}
if(-not $signtool){throw 'signtool.exe not found. Install the Windows SDK signing tools.'}
# Refuse to write an unprotected release package to a public network share accidentally.
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$out=(Resolve-Path -LiteralPath $OutputDirectory).Path
if($out.StartsWith('\\')){throw 'Use a local signing folder, not a UNC share.'}
$selfSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
& icacls.exe $out /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' ("${selfSid}:(OI)(CI)F") | Out-Null
if($LASTEXITCODE -ne 0){throw 'Failed to protect release folder ACL'}
$stage=Join-Path $out 'VGStatusbar-Windows-x64'
if(Test-Path $stage){Remove-Item -LiteralPath $stage -Recurse -Force}
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'vg-status-client.exe') -Force
$destExe=Join-Path $stage 'vg-status-client.exe'
$args=@('sign','/sha1',$thumb,'/s','My','/fd','SHA256','/d','VG Statusbar',$destExe)
if($TimestampUrl){$args=@('sign','/sha1',$thumb,'/s','My','/fd','SHA256','/tr',$TimestampUrl,'/td','SHA256','/d','VG Statusbar',$destExe)}
& $signtool @args
if($LASTEXITCODE -ne 0){throw 'SignTool sign failed'}
& $signtool verify /pa /v $destExe
if($LASTEXITCODE -ne 0){throw 'SignTool verify failed'}
# Preserve config as UTF-8 BOM-free. Current client uses a readable shared token.
$configContent=Get-Content -LiteralPath $cfg -Raw
[System.IO.File]::WriteAllText((Join-Path $stage 'client.json'),$configContent,(New-Object System.Text.UTF8Encoding($false)))
Copy-Item (Join-Path $PSScriptRoot 'Install-VGStatusbar.ps1') $stage -Force
Copy-Item (Join-Path $PSScriptRoot 'Uninstall-VGStatusbar.ps1') $stage -Force
Copy-Item (Join-Path $PSScriptRoot 'Restore-LastBackup.ps1') $stage -Force
$hash=(Get-FileHash -LiteralPath $destExe -Algorithm SHA256).Hash
Set-Content -LiteralPath (Join-Path $stage 'SHA256.txt') -Value $hash -Encoding ASCII
$zip=Join-Path $out 'VGStatusbar-PRIVATE-Deployment.zip'
if(Test-Path $zip){Remove-Item $zip -Force}
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
Write-Host ('Signed release: ' + $destExe)
Write-Host ('PRIVATE DEPLOYMENT ZIP (includes client token): ' + $zip)
Write-Warning 'The ZIP contains a shared client token. Restrict access; never publish or attach to a public GitHub release.'
Write-Warning 'Internal AD CS signing does not guarantee bypassing SmartScreen or endpoint policies.'
