# Notiflag for Grafana alerting

A firing Grafana alert rule opens a message on your users' desktops; when the rule returns to *Normal*,
Grafana's resolved notification gives the message its green all-clear and takes it away.
Notiflag reads Grafana's webhook format as it is.

| File | Purpose |
|---|---|
| [`contact-point.yaml`](contact-point.yaml) | File provisioning of the contact point, token read from a file |
| [`test-firing.json`](test-firing.json), [`test-resolved.json`](test-resolved.json) | Real Grafana payloads for testing with `curl` |

Tested with Grafana 13.0: contact point created in the UI/API and by file provisioning, a real alert rule
firing and resolving, and the contact point's *Test* button.

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `Grafana`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Grafana

### 2.1 Contact point (UI)

*Alerting → Contact points → + Add contact point*:

1. Name: `Notiflag`
2. Integration: **Webhook**
3. URL: `https://status.example.com/api/hooks` (without the token)
4. Open *Optional Webhook settings*:
   * HTTP Method: `POST`
   * Authorization Header – Scheme: `Bearer`
   * Authorization Header – Credentials: the token (`sb_…`, the part after `/api/hooks/`)

   Simpler alternative: put the whole webhook URL with token into *URL* and leave the authorization empty.
   The header keeps the token out of Grafana's and the proxy's logs.
5. *Notification settings*: leave **Disable resolved message** off – the resolved message takes the
   Notiflag message down.
6. *Test* (see step 4), then *Save contact point*.

### 2.1 (alternative) Contact point by file provisioning

```bash
install -m 640 -o root -g grafana /dev/null /etc/grafana/notiflag.token
printf '%s\n' 'sb_0123…' > /etc/grafana/notiflag.token
cp contact-point.yaml /etc/grafana/provisioning/alerting/notiflag.yaml   # adjust the URL inside
systemctl restart grafana-server
```

`$__file{/etc/grafana/notiflag.token}` in the file reads the token at start-up. In Docker/Kubernetes
mount the token as a secret file to the same path.

### 2.2 Send the right alerts to Notiflag

Either per alert rule (simplest, Grafana 11 and later):

* Alert rule → *Configure notifications* → **Contact point: Notiflag**.

Or centrally with a notification policy:

* Give the user-facing alert rules a label `audience = users`.
* *Alerting → Notification policies → + New child policy*: matcher `audience = users`, contact point
  **Notiflag**, enable *Continue matching subsequent sibling nodes* so on-call still gets them.

### 2.3 Write the alert rule for people

| Rule field | Use |
|---|---|
| Rule name | becomes `alertname`, shown if there is no summary |
| Annotation **Summary** | message title, e.g. `The intranet is not reachable` |
| Annotation **Description** | message text, e.g. `We are working on it.` |
| Label **severity** | `critical` → Outage, `warning` → Degraded, `info` → Notice; missing → Outage |
| *Configure no data and error handling* | set *Alert state if no data* and *if execution error* to **Normal** or **Keep last state** – otherwise users get messages titled `DatasourceNoData` / `DatasourceError` |

## 3. How Grafana maps to Notiflag

| Grafana | Notiflag |
|---|---|
| `alerts[].status` = `firing` | problem: message opens |
| `alerts[].status` = `resolved` | recovery: message resolved |
| `alerts[].fingerprint` | key |
| annotation `summary` (or `title`) | title; without it the rule name (`alertname`) |
| annotation `description` (or `message`) | message text |
| label `severity` | see table above |
| label `instance` (or `host`, `hostname`, `node`) | host – often empty in Grafana |
| label `service`, else `job`, else `alertname` | service |

A multi-dimensional rule (one alert per series) creates one message per series, each resolved separately.
Repeats (`repeat_interval`) are answered with `duplicate`.

## 4. Test

**Test button** – on the contact point, *Test*. Grafana sends a firing alert titled
**Notification test**; Notiflag answers with a draft (or message, depending on the policy). Grafana
never sends the matching *resolved* for a test, so discard it under *Messages → Waiting for approval*
(or *Resolve* it).

**Sample payloads** – a full cycle without Grafana:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-firing.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-resolved.json
# {"events":["cancelled"],"ok":true}
```

**End to end** – a rule that always fires, e.g. with the built-in *TestData* data source
(scenario *CSV Metric Values*, values `1,1,1`, condition *IS ABOVE 0*), contact point Notiflag.
Change the values to `0,0,0` to let it resolve. Mind the rule group's evaluation interval and the policy's
*group wait* (30 s by default).

## 5. Example rule

For alerts whose texts you cannot change (Notiflag, *Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `*/Intranet*` |
| Show this title | `The intranet is not reachable` |
| And this message | `We are working on it.` |
| Severity | As reported |

Without an `instance` label the subject is `/<rule name>`, so `*/Intranet*` matches a rule named
"Intranet unreachable". Matching on the title also works: `*intranet*`.

## Troubleshooting

Contact points show the last delivery status and error (*Alerting → Contact points*, health column).
Server log: `logger=ngalert.notifier.webhook` / `level=error`.

| Symptom | Fix |
|---|---|
| `Unknown integration token` / status 401 | Credentials field contains the whole URL or `Bearer sb_…`; enter the bare token. Or the integration was deleted. |
| `x509: certificate signed by unknown authority` | Install your internal root CA in the Grafana host/container trust store, or set the contact point's *TLS* → CA certificate. |
| Message never resolves | *Disable resolved message* is on, or the alert was silenced/muted when it resolved. |
| Title `DatasourceNoData` / `DatasourceError` | No-data/error handling of the rule; set it to *Normal* or *Keep last state*. |
| Nothing arrives | The rule is not routed to Notiflag: check *Configure notifications* / the policy tree (*Alerting → Notification policies*, preview of matching policies in the rule). |
| Changes to the contact point seem ignored | Grafana applies alerting configuration changes asynchronously (up to a minute). |
