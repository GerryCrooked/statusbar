<#
.SYNOPSIS
    Notiflag notification for PRTG ("Execute Program"), also usable from any Windows script or task.

.DESCRIPTION
    Copy to the PRTG core server:
        C:\Program Files (x86)\PRTG Network Monitor\Notifications\EXE\notiflag.ps1
    Put the webhook URL https://status.example.com/api/hooks/<token> alone into
        C:\ProgramData\Notiflag\webhook-url.txt
    (or pass -Url). Then create a notification template with "Execute Program":
        Program file:  notiflag.ps1
        Parameters:    -Status '%status' -Device '%device' -Sensor '%name' -SensorId '%sensorid' -Message '%message'
    and use it in a state trigger for "Down" (or "Warning") AND as the notification
    "when the condition clears". Guide: integrations/prtg/README.md

    PRTG states and what Notiflag gets:
        Down, Down (Partial)          -> problem, severity error
        Warning, Unusual              -> problem, severity warning
        Up                            -> recovery
        Down (Acknowledged), Paused…  -> not sent (exit 0)

    Works with Windows PowerShell 5.1 and PowerShell 7.
#>
param(
    [string]$Status = '',
    [string]$Device = '',
    [string]$Sensor = '',
    [string]$SensorId = '',
    [string]$Message = '',
    [string]$Key = '',
    [string]$Severity = '',
    [string]$Url = '',
    [string]$UrlFile = ''
)
$ErrorActionPreference = 'Stop'

function Fail([string]$text, [int]$code) {
    # PRTG shows the output of a failed program in the notification log.
    Write-Output "Notiflag: $text"
    exit $code
}

if (-not $Url) {
    if (-not $UrlFile) { $UrlFile = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Notiflag\webhook-url.txt' }
    if (-not (Test-Path -LiteralPath $UrlFile)) { Fail "no -Url given and $UrlFile not found" 2 }
    $Url = (Get-Content -LiteralPath $UrlFile -TotalCount 1).Trim()
}
if ($Url -notmatch '^https?://') { Fail 'the webhook URL must start with https://' 2 }

$s = $Status.Trim().ToLowerInvariant()
if ($s -like '*acknowledged*' -or $s -like 'paused*' -or $s -eq '' -or $s -eq 'unknown') {
    Write-Output "Notiflag: skipped state '$Status'"
    exit 0
}
if ($s -like 'down*')                              { $state = 'down';    $level = 'error' }
elseif ($s -like 'warning*' -or $s -like 'unusual*') { $state = 'warning'; $level = 'warning' }
elseif ($s -like 'up*')                            { $state = 'up';      $level = '' }
else                                               { $state = $Status;   $level = '' }

$body = [ordered]@{
    key      = if ($Key) { $Key } elseif ($SensorId) { "prtg-$SensorId" } else { "prtg-$Device/$Sensor" }
    status   = $state
    host     = $Device
    service  = $Sensor
    message  = $Message
    severity = if ($Severity) { $Severity } else { $level }
}
$json = $body | ConvertTo-Json -Compress

# Windows PowerShell 5.1 defaults to TLS 1.0 and Latin-1 bodies: force TLS 1.2 and UTF-8.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
try {
    $answer = Invoke-RestMethod -Method Post -Uri $Url -TimeoutSec 10 -UseBasicParsing `
        -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($json))
    Write-Output ("Notiflag: " + ($answer | ConvertTo-Json -Compress))
    exit 0
} catch {
    $detail = $_.ErrorDetails.Message
    if (-not $detail) { $detail = $_.Exception.Message }
    Fail "webhook failed: $detail" 1
}
