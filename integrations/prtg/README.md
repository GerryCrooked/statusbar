# Notiflag for PRTG

A PRTG sensor going *Down* (or *Warning*) opens a message on your users' desktops; when it is *Up* again,
the message gets its green all-clear and disappears.

| File | Purpose |
|---|---|
| [`notiflag.ps1`](notiflag.ps1) | **Recommended.** Notification script for *Execute Program*. Builds correct JSON from any sensor message, handles all PRTG states. Windows PowerShell 5.1 or PowerShell 7. |
| [`http-action-payload.txt`](http-action-payload.txt) | Payload for the built-in *Execute HTTP Action*, if you cannot place a script on the PRTG server. |

Tested: the script with PowerShell 7 against a throwaway Notiflag server (Down, Down (Partial),
Down (Acknowledged), Warning, Up, sensor messages with quotes and umlauts, wrong token, missing URL file);
the HTTP action payload with PRTG's form content type. PRTG itself runs on Windows only and was not part of
the automated test.

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `PRTG`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up PRTG – with the PowerShell script (recommended)

### 2.1 Install the script on the PRTG core server

1. Copy `notiflag.ps1` to
   `C:\Program Files (x86)\PRTG Network Monitor\Notifications\EXE\notiflag.ps1`
   (with a failover cluster: on every cluster node).
2. Store the webhook URL in a file only administrators and the PRTG service can read, in an elevated PowerShell:

   ```powershell
   New-Item -ItemType Directory -Force C:\ProgramData\Notiflag | Out-Null
   Set-Content -Path C:\ProgramData\Notiflag\webhook-url.txt -Value 'https://status.example.com/api/hooks/<token>' -Encoding ASCII
   icacls C:\ProgramData\Notiflag /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F'
   ```

   The PRTG core server service runs as *Local System* by default; if it runs under another account,
   grant that account read access too.
3. If PowerShell scripts are blocked on the server, allow local scripts once. PRTG may start `.ps1`
   notifications with the 32-bit Windows PowerShell, so run this in an elevated 64-bit **and** an elevated
   32-bit PowerShell (`C:\Windows\SysWOW64\WindowsPowerShell\v1.0\powershell.exe`):

   ```powershell
   Set-ExecutionPolicy RemoteSigned -Scope LocalMachine
   ```

   Copied files may be marked as downloaded: `Unblock-File` on `notiflag.ps1`.

### 2.2 Notification template

*Setup → Account Settings → Notification Templates → Add Notification Template*:

1. Template name `Notiflag`. Leave *Notification Summarization* at **Always notify ASAP, never summarize**
   – summarized notifications carry no single sensor state.
2. Enable **Execute Program**:
   * Program file: `notiflag.ps1`
   * Parameters:
     ```
     -Status '%status' -Device '%device' -Sensor '%name' -SensorId '%sensorid' -Message '%message'
     ```
   * Timeout: `30`
3. *Save*.

### 2.3 Notification triggers

On the probe, group, device or sensor whose outage users should hear about, tab *Notification Triggers* →
*Add State Trigger*:

* When sensor state is **Down** for at least **60** seconds, perform **Notiflag**
* When sensor state is **Down** for at least 300 seconds, perform **(none)** (no escalation needed)
* When the **condition clears**, perform **Notiflag** – **this sends the recovery; without it the
  message stays.**

Add a second state trigger for **Warning** the same way if users should also see degraded services.

Triggers are inherited: one on a group covers all its devices and sensors. To keep noise low, put them on
the few sensors users notice (mail, intranet, ERP, VPN) rather than on the root group.

## 2 (alternative). Set up PRTG – with the HTTP action

Use this only if no script can be placed on the PRTG server. The payload contains no free text, because
PRTG inserts placeholders into the JSON without escaping, and a sensor message with a quote (`can't
connect`) would make the JSON invalid (`400 JSON body required`).

Notification template → enable **Execute HTTP Action**:

* URL: the webhook URL from step 1
* HTTP method: `POST`
* Payload (copy from `http-action-payload.txt`):
  ```json
  {"key":"prtg-%sensorid","status":"%status","host":"%device","service":"%name"}
  ```

Use the same triggers as above. With the HTTP action only the states `Down`, `Warning` and `Up` are
understood; `Down (Partial)` and `Unusual` are ignored. Use a rule in Notiflag to give users a message
text.

## 3. How PRTG maps to Notiflag

| PRTG `%status` | Script sends | HTTP action |
|---|---|---|
| Down, Down (Partial) | problem, Outage (`error`) | Down: problem; Down (Partial): ignored |
| Warning, Unusual | problem, Degraded (`warning`) | Warning: problem; Unusual: ignored |
| Up | recovery | recovery |
| Down (Acknowledged), Paused…, Unknown | nothing (skipped) | ignored |
| `%sensorid` | key `prtg-<id>` | same |
| `%device` / `%name` | host / service; title without rule: `SMTP (mail01.example.local)` | same |
| `%message` | message text without rule | – |

If a sensor goes from *Down* to *Warning*, the *Warning* is a problem with the same key: Notiflag answers
`duplicate` and the message stays until the sensor is *Up*.

## 4. Test

**On the PRTG server** – the same call PRTG makes, in a PowerShell window:

```powershell
cd 'C:\Program Files (x86)\PRTG Network Monitor\Notifications\EXE'
.\notiflag.ps1 -Status 'Down' -Device 'prtg-test' -Sensor 'Notiflag test' -SensorId 'test' -Message 'This is a test'
# Notiflag: {"events":["draft"],"ok":true}
.\notiflag.ps1 -Status 'Up' -Device 'prtg-test' -Sensor 'Notiflag test' -SensorId 'test'
# Notiflag: {"events":["cancelled"],"ok":true}
```

**From PRTG** – *Notification Templates*, select *Notiflag*, *Send test notification*. PRTG fills the
placeholders with sample values. If the sample status is a real state such as *Down*, a draft (or message)
appears in Notiflag – discard it under *Messages → Waiting for approval* (or *Resolve* it), a test never
sends the matching *Up*. If the log shows `Notiflag: skipped state '…'`, the sample status was not a
sensor state; the script works, use the command-line test above for a full cycle.

**End to end** – pause/resume is not enough (paused is skipped). Use a test sensor, e.g. a *Ping* sensor
to an unused IP address, give it the trigger, and watch the message appear after the trigger's delay.

## 5. Example rule

In Notiflag (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `*/HTTP*Intranet*` |
| Show this title | `The intranet is not reachable` |
| And this message | `We are working on it.` |
| Severity | As reported |

Or by sensor ID, if names change: pattern `prtg-2041` (the key) → `E-mail is disrupted`.

## Troubleshooting

Notification results are in *Logs → System Events → Notifications* and in the sensor's *Log* tab.
The script's output (`Notiflag: …`) is shown there when it fails.

| Symptom | Fix |
|---|---|
| `Notiflag: no -Url given and C:\ProgramData\Notiflag\webhook-url.txt not found` | Create the file (step 2.1), or check that the PRTG service account can read it. |
| `Notiflag: webhook failed: Unknown integration token` | The URL in the file is incomplete or the integration was deleted. |
| `Notiflag: webhook failed: The underlying connection was closed` / `Could not establish trust relationship` | The PRTG server does not trust your Notiflag certificate: import your internal root CA into *Local Computer → Trusted Root Certification Authorities*. |
| `… cannot be loaded because running scripts is disabled` | Execution policy (step 2.1, point 3) – set it for the 32-bit PowerShell too. |
| `Notiflag: skipped state 'Down (Acknowledged)'` | Expected: acknowledging in PRTG changes nothing on users' screens. |
| Message stays after the sensor is Up | The trigger has no *condition clears* action. |
| HTTP action: `400 JSON body required` | A placeholder contained a quote. Use the script, or remove the field from the payload. |
| HTTP action: response is `{"events":[],"ok":true}` | The state was `Down (Partial)`, `Unusual` or another word Notiflag does not know. Use the script. |
