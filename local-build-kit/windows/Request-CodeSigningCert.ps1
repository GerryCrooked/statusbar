# Run as the designated signing user on a domain-joined Windows machine.
# The AD CS certificate template must be published and allowed for this account.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Template)
$ErrorActionPreference='Stop'
$cert = Get-Certificate -Template $Template -CertStoreLocation 'Cert:\CurrentUser\My' -ErrorAction Stop
if (-not $cert.Certificate) { throw 'AD CS did not return a certificate. Check template publish/enroll permissions.' }
$c = $cert.Certificate
$codeSigningEku='1.3.6.1.5.5.7.3.3'
$eku=@($c.EnhancedKeyUsageList | ForEach-Object ObjectId)
if ($eku -notcontains $codeSigningEku) { throw 'Issued certificate lacks Code Signing EKU.' }
if (-not $c.HasPrivateKey) { throw 'No private key available for this user.' }
Write-Host ('Certificate Thumbprint: ' + $c.Thumbprint)
Write-Host ('Subject: ' + $c.Subject)
Write-Host ('Expires: ' + $c.NotAfter)
Write-Host 'Keep private key non-exportable. Do not copy CA keys or PFX to Linux.'
