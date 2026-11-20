# Statusbar with other RMMs: ConnectWise, Kaseya VSA, N-sight and more

Some RMMs have no outgoing webhook for alerts, or one that cannot be shaped:

| RMM | What it offers for alerts | Recommended way |
|---|---|---|
| **ConnectWise Automate** | alert templates: e-mail, ticket, run a script | A, or B on a probe/server |
| **ConnectWise RMM** | notifications by e-mail, ticket, automation | A |
| **Kaseya VSA** (9.x, 10) | alarm e-mails, agent procedures; VSA 10 webhooks only for MDM events | A, or B |
| **N-able N-sight RMM** | alert e-mails, automated tasks | A |
| ManageEngine Endpoint Central, Syncro, Pulseway … | alert e-mails, scripts | A or B |

N-able **N-central**, **NinjaOne**, **Datto RMM**, **Tactical RMM** and **Atera** have their own guides
in this folder's siblings.

**Not tested in these products** – both ways reuse parts tested elsewhere in this repository
(the Power Automate flow's payload, `statusbar.ps1`, `statusbar-notify.sh`).

## A. Alert e-mails (works with every RMM, also when the device is offline)

Every RMM can e-mail its alerts, including "back to normal". Send them to a shared mailbox and let a flow
turn them into messages:

1. In Statusbar: an integration `RMM alerts` (*Hold for approval*, delay `0`).
2. In the RMM: alert e-mails for the devices users depend on to `rmm-alerts@example.com`, with the
   **recovery / alert cleared** e-mail switched on. Keep the subject predictable, e.g.
   `ALERT: <device> - <monitor>` and `CLEARED: <device> - <monitor>` (most RMMs have subject templates).
3. A Power Automate flow on that mailbox as in [power-automate](../power-automate/README.md):
   * subject starts with `ALERT:` → body with `"status": "problem"`
   * subject starts with `CLEARED:` → body with `"status": "ok"`
   * `key`: the subject after the prefix, e.g. expression
     `trim(substring(triggerOutputs()?['body/subject'], indexOf(triggerOutputs()?['body/subject'], ':') + 1))`
     – identical for alert and cleared mail of the same monitor.

No Microsoft 365? Any mail-to-webhook tool works the same way (n8n, a procmail/fetchmail script calling
[`statusbar-notify.sh`](../curl/statusbar-notify.sh)).

## B. A script run by the RMM

Most RMMs can run a script when an alert fires and when it clears. Use the bundled senders:

* Windows: [`statusbar.ps1`](../prtg/statusbar.ps1)
  ```powershell
  .\statusbar.ps1 -Status Down -Device 'FS01' -Sensor 'Disk C:' -Key 'rmm-FS01-disk' -UrlFile C:\ProgramData\Statusbar\webhook-url.txt
  .\statusbar.ps1 -Status Up   -Device 'FS01' -Sensor 'Disk C:' -Key 'rmm-FS01-disk' -UrlFile C:\ProgramData\Statusbar\webhook-url.txt
  ```
* Linux/macOS: [`statusbar-notify.sh`](../curl/statusbar-notify.sh)
  ```bash
  STATUSBAR_URL_FILE=/etc/statusbar/webhook-url STATUSBAR_KEY=rmm-fs01-disk statusbar-notify.sh CRITICAL fs01 "Disk /srv"
  STATUSBAR_URL_FILE=/etc/statusbar/webhook-url STATUSBAR_KEY=rmm-fs01-disk statusbar-notify.sh OK fs01 "Disk /srv"
  ```

Insert the RMM's own variables for device and monitor (ConnectWise Automate: `%ComputerName%`; Kaseya
agent procedures: `#vMachine.machName#` …) and keep the key identical for alert and clear.

**Limit**: a script on the affected device cannot run while that device is offline or unreachable – the
most important case. Run the script on a **probe or management server** instead (ConnectWise Automate:
the location's probe; Kaseya: a dedicated "monitoring" agent), or use way A.

## Test

Send the two calls by hand first (the guides above show `curl` and PowerShell examples): the message
must appear and disappear. Then trigger one real alert in the RMM.

## Troubleshooting

See [docs/INTEGRATIONS.md → Troubleshooting](../../docs/INTEGRATIONS.md#troubleshooting). The most common
issue is a key that differs between alert and clear – the message then stays; *Resolve* it in Statusbar
and make both keys identical.
