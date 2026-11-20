# Statusbar for Checkmk

A Checkmk service going CRIT (or a host going DOWN) opens a message on your users' desktops; when it is
OK/UP again, the message gets its green all-clear and disappears.

| File | Purpose |
|---|---|
| [`statusbar`](statusbar) | Notification script (Python 3, no extra modules), Checkmk 2.0 and later, all editions |

Tested on a Checkmk 2.4 Raw site: service and host problems, recoveries, skipped acknowledgements,
exit codes for wrong token and unreachable server.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Checkmk`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Checkmk

### 2.1 Install the script

As the **site user** on the Checkmk server (in a distributed setup: on every site that sends notifications):

```bash
su - mysite
cp /path/to/statusbar ~/local/share/check_mk/notifications/statusbar
chmod 755 ~/local/share/check_mk/notifications/statusbar
```

No restart is needed. The second line of the file (`# Statusbar`) is the name Checkmk shows as
notification method.

### 2.2 Notification rule

*Setup → Events → Notifications → Add notification rule*. In Checkmk 2.3/2.4 this is a guided form; in
2.0–2.2 a single page with the same sections.

1. **Triggering events** – include both directions, otherwise messages never go away:
   * Host events: *Up → Down*, *Down → Up* (or *Any → Down*, *Any → Up*)
   * Service events: *Any → Critical*, *Any → OK*; add *Any → Warning* only if users should see warnings.
   * Leave *Acknowledgement*, *Downtime*, *Flapping* off – the script skips them anyway.
2. **Filters** – only what users should hear about, e.g. *Match host labels* `audience:users`, *Match host
   groups* `mail-servers`, or *Match services* `SMTP|HTTPS Intranet`.
3. **Notification method**: **Statusbar**, *Call with the following parameters*:
   * Parameter 1: **the webhook URL** from step 1
4. **Recipients**: a single dedicated user (e.g. create a user `statusbar` without login and choose
   *Specify users*). The script runs once per recipient; with several recipients Statusbar answers
   `duplicate` for the extra calls, which is harmless but noisy.
5. **Conditions**: none needed. *Save*, then *Activate pending changes*.

## 3. How Checkmk maps to Statusbar

| Checkmk | Statusbar |
|---|---|
| `NOTIFY_NOTIFICATIONTYPE` = `PROBLEM` | problem: message opens |
| `NOTIFY_NOTIFICATIONTYPE` = `RECOVERY` | recovery: message resolved |
| `ACKNOWLEDGEMENT`, `DOWNTIMESTART`, `FLAPPINGSTART`, `CUSTOM` … | skipped by the script (exit 0) |
| Service state `WARNING` | Degraded (`warning`) |
| Service `CRITICAL`, `UNKNOWN`, host `DOWN`, `UNREACHABLE` | Outage (`error`) |
| Host name / service description | host / service, key `host/service`; host problems key `host/` |
| Plugin output (`NOTIFY_SERVICEOUTPUT`, `NOTIFY_HOSTOUTPUT`) | message text without rule |

A change from WARN to CRIT is a second `PROBLEM`; Statusbar answers `duplicate` and keeps the first message.

Exit codes follow Checkmk's convention: `0` delivered or skipped, `1` temporary error (network – Checkmk
retries), `2` permanent error (wrong URL or token – no retry).

## 4. Test

**From the command line** – as the site user, without touching any host:

```bash
NOTIFY_PARAMETER_1='https://status.example.com/api/hooks/<token>' NOTIFY_NOTIFICATIONTYPE=PROBLEM \
NOTIFY_WHAT=SERVICE NOTIFY_HOSTNAME=checkmk-test NOTIFY_SERVICEDESC=Statusbar NOTIFY_SERVICESTATE=CRITICAL \
NOTIFY_SERVICEOUTPUT='This is a test' ~/local/share/check_mk/notifications/statusbar
# Statusbar: {"events":["draft"],"ok":true}

NOTIFY_PARAMETER_1='https://status.example.com/api/hooks/<token>' NOTIFY_NOTIFICATIONTYPE=RECOVERY \
NOTIFY_WHAT=SERVICE NOTIFY_HOSTNAME=checkmk-test NOTIFY_SERVICEDESC=Statusbar NOTIFY_SERVICESTATE=OK \
~/local/share/check_mk/notifications/statusbar
# Statusbar: {"events":["cancelled"],"ok":true}
```

**Through Checkmk** – open a service your rule covers, *Commands → Fake check results → Critical*.
The notification goes out at once; the next regular check (or *Fake check results → OK*) sends the recovery.
In 2.3 and later, *Setup → Events → Test notifications* simulates an event and shows which rules match.

## 5. Example rule

In Statusbar (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `*/HTTPS Intranet*` |
| Show this title | `The intranet is not reachable` |
| And this message | `We are working on it. Shared drives and e-mail are not affected.` |
| Severity | Outage |

And one for whole hosts: `erp-db1/` (host problems have an empty service) → `The ERP system is unavailable`.

## Troubleshooting

All notification activity is logged in `~/var/log/notify.log` on the site. For more detail set
*Setup → General → Global settings → Notifications → Notification log level* to *Full dump of all variables*.

| In `notify.log` / symptom | Fix |
|---|---|
| `Statusbar: parameter 1 must be the webhook URL` | Parameter 1 of the rule is empty or not a URL. |
| `Statusbar answered HTTP 401: Unknown integration token` | URL incomplete or integration deleted. |
| `Statusbar webhook failed: <urlopen error [Errno -2] Name or service not known>` | The Checkmk server cannot resolve/reach Statusbar. Test with `curl` as the site user. A proxy is taken from `https_proxy` in the site's environment (`~/etc/environment`). |
| `CERTIFICATE_VERIFY_FAILED` | Install your internal root CA on the Checkmk server (`update-ca-certificates`) – the site's Python uses the system store. |
| Method *Statusbar* missing in the rule | Script not in `~/local/share/check_mk/notifications/` of the site, or not executable. |
| Problem arrives, recovery does not | *Triggering events* lack *→ OK* / *→ Up*. |
| `Statusbar: skipped notification type …` | Expected for acknowledgements, downtimes and flapping. |
| Nothing in `notify.log` | Notifications are switched off globally (*Commands → Notifications* in the master control snap-in) or no rule matches – check with *Test notifications* / *Analyze recent notifications*. |
