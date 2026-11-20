# Statusbar with curl, scripts and anything else

Everything that can make an HTTP request can put a message on your users' screens and take it down
again: cron jobs, backup software, deployment pipelines, home-grown checks, Windows scheduled tasks.

| File | Purpose |
|---|---|
| [`statusbar-notify.sh`](statusbar-notify.sh) | Sender for the shell (POSIX `sh` + `curl`): `statusbar-notify.sh <status> <host> [service] [message]` |
| [`statusbar-run.sh`](statusbar-run.sh) | Wraps a job: failure opens a message, the next successful run resolves it |
| [`test-problem.json`](test-problem.json), [`test-recovery.json`](test-recovery.json) | Generic payloads for `curl --data-binary @…` |
| [`../prtg/statusbar.ps1`](../prtg/statusbar.ps1) | The same for Windows/PowerShell (`-Status -Device -Sensor -Message -Key -Url`) |

Tested with bash, dash and busybox `sh` against a throwaway Statusbar server, including messages with
quotes, backslashes, line breaks and tabs.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name e.g. `Scripts` or `Backup`, policy *Hold for approval*, delay `0`
for jobs (they do not flap) or `60` for checks. Copy the webhook URL – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Plain curl

```bash
URL='https://status.example.com/api/hooks/<token>'

# problem
curl -fsS -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"fs01","service":"Backup","message":"Last night'"'"'s backup failed"}'
# {"events":["draft"],"ok":true}

# recovery – same host and service (or the same "key")
curl -fsS -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"fs01","service":"Backup"}'
# {"events":["cancelled"],"ok":true}  (or "resolved" once it was published)
```

Or with the sample files: `curl -fsS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-problem.json`.

Tips:

* `-f` makes curl exit with code 22 on HTTP errors, so scripts notice a wrong token.
* Building JSON by hand breaks as soon as a value contains `"` or `\`. Use `jq -n --arg …`, the
  script below, or a language with a JSON library.
* Without a token in the URL: `-H "Authorization: Bearer $TOKEN"` to `https://status.example.com/api/hooks`.
* All fields, status words and outcomes: [Generic payload reference](../../docs/INTEGRATIONS.md#generic-payload-reference).

With `jq` for safe quoting:

```bash
jq -n --arg host "$(hostname)" --arg msg "$error_text" \
  '{status:"problem", host:$host, service:"Backup", message:$msg, severity:"warning"}' |
  curl -fsS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @-
```

## 3. statusbar-notify.sh

```bash
install -m 755 statusbar-notify.sh /usr/local/bin/statusbar-notify.sh
install -d -m 700 /etc/statusbar
echo 'https://status.example.com/api/hooks/<token>' > /etc/statusbar/webhook-url
chmod 600 /etc/statusbar/webhook-url

export STATUSBAR_URL_FILE=/etc/statusbar/webhook-url
statusbar-notify.sh CRITICAL fs01 Backup "Disk full while writing"
statusbar-notify.sh OK fs01 Backup
```

| Variable | Meaning |
|---|---|
| `STATUSBAR_URL` or `STATUSBAR_URL_FILE` | the webhook URL, or a file whose first line is the URL (keeps it out of `ps` and logs) |
| `STATUSBAR_KEY` | stable ID; default `host/service` |
| `STATUSBAR_SEVERITY` | `error`, `warning` or `info` |
| `STATUSBAR_TITLE` | title shown without rule |
| `STATUSBAR_TYPE` | notification type from a monitoring system; only `PROBLEM` and `RECOVERY` are sent |

The script escapes the values for JSON, retries twice on network errors and exits non-zero if the
message was not delivered.

## 4. statusbar-run.sh – watch a job

```bash
install -m 755 statusbar-run.sh /usr/local/bin/statusbar-run.sh
```

Crontab:

```
30 2 * * *  STATUSBAR_URL_FILE=/etc/statusbar/webhook-url /usr/local/bin/statusbar-run.sh backup-fs01 "File server backup" -- /usr/local/bin/backup.sh
```

* Exit code ≠ 0: message *File server backup* (Degraded, text: exit code and the last three output lines).
* Exit code 0: resolves it (or answers `ignored` if nothing was open).
* The job's output and exit code are passed through, so cron mail and logging work as before.

## 5. Windows

```powershell
.\statusbar.ps1 -Status Down -Device $env:COMPUTERNAME -Sensor 'Backup' -Message 'Backup failed' -Key 'backup-fs01' -UrlFile C:\ProgramData\Statusbar\webhook-url.txt
.\statusbar.ps1 -Status Up   -Device $env:COMPUTERNAME -Sensor 'Backup' -Key 'backup-fs01' -UrlFile C:\ProgramData\Statusbar\webhook-url.txt
```

Or directly:

```powershell
$body = @{ status = 'problem'; host = $env:COMPUTERNAME; service = 'Backup'; message = 'Backup failed' } | ConvertTo-Json
Invoke-RestMethod -Method Post -Uri $url -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))
```

## 6. Example rule

In Statusbar (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `backup-*` |
| Show this title | `Last night's backup did not finish` |
| And this message | `Files changed since yesterday are not yet backed up. IT is on it.` |
| Severity | Degraded |
| Recipients | e.g. only the IT team |

`backup-*` matches the key of every job started with `statusbar-run.sh backup-…`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `curl: (22) The requested URL returned error: 401` | URL incomplete or integration deleted. |
| `curl: (22) … error: 400` | Invalid JSON – check quoting; try the same call with `-v` without `-f` to see `JSON body required`. |
| `curl: (60) SSL certificate problem` | Install your internal root CA (`update-ca-certificates`); `--cacert root-ca.pem` for a quick test. |
| `{"events":[],"ok":true}` | `status` was not a problem or recovery word, or none of `host`, `service`, `key`, `title` was set. |
| Recovery answers `ignored` | Different `host`/`service`/`key` than the problem, or sent to a different integration. |
