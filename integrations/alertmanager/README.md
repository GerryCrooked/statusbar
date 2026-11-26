# Notiflag for Prometheus Alertmanager

A firing alert opens a message on your users' desktops; when Alertmanager sends it as *resolved*, the
message gets its green all-clear and disappears. Notiflag reads Alertmanager's native webhook format –
no adapter in between.

| File | Purpose |
|---|---|
| [`alertmanager.yml`](alertmanager.yml) | Receiver and route for Notiflag, token read from a file |
| [`notiflag-rules.yml`](notiflag-rules.yml) | Example Prometheus alert rules written for end users |
| [`test-firing.json`](test-firing.json), [`test-resolved.json`](test-resolved.json) | Real Alertmanager payloads for testing with `curl` |

Tested with Alertmanager 0.28.1 (config check with `amtool`, alert pushed through the API, firing and
resolved delivered with the Bearer token from a file).

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `Alertmanager`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Alertmanager

1. Put **only the token** (the part after `/api/hooks/`, starting with `sb_`) into a file:

   ```bash
   printf '%s\n' 'sb_0123…' > /etc/alertmanager/notiflag.token
   chown alertmanager: /etc/alertmanager/notiflag.token; chmod 600 /etc/alertmanager/notiflag.token
   ```

   (In Kubernetes: a Secret mounted as a file, e.g. via the operator's `AlertmanagerConfig` with
   `authorization.credentials` referencing the Secret.)

2. Add the receiver and a route from `alertmanager.yml` to your configuration:

   ```yaml
   route:
     receiver: default
     routes:
       - matchers: ['audience="users"']
         receiver: notiflag
         group_by: ['alertname', 'instance']
         continue: true

   receivers:
     - name: notiflag
       webhook_configs:
         - url: https://status.example.com/api/hooks
           send_resolved: true
           http_config:
             authorization:
               type: Bearer
               credentials_file: /etc/alertmanager/notiflag.token
   ```

   * `send_resolved: true` is required, otherwise messages never go away.
   * Notiflag handles every alert in the group individually, so grouping only affects timing.
   * `continue: true` lets the alert also reach the routes after this one (your on-call receivers).
   * Alternative without token file: `url: https://status.example.com/api/hooks/sb_…`.

3. Check and reload:

   ```bash
   amtool check-config /etc/alertmanager/alertmanager.yml
   curl -X POST http://localhost:9093/-/reload        # or: systemctl reload alertmanager
   ```

4. Decide which alerts users see. The example routes on a label `audience="users"`; add it to the
   Prometheus rules meant for users (see `notiflag-rules.yml`) and load them via `rule_files:`.
   Write `summary` and `description` for people, not for admins – Notiflag shows them as title and text.

## 3. How Alertmanager maps to Notiflag

| Alertmanager | Notiflag |
|---|---|
| `alerts[].status` = `firing` | problem: message opens |
| `alerts[].status` = `resolved` | recovery: message resolved |
| `alerts[].fingerprint` | key (stable for the alert's label set) |
| `annotations.summary` (or `title`) | title; without it `alertname (instance)` |
| `annotations.description` (or `message`) | message text |
| `labels.severity` | `critical`, `page`, `high`, `major` → Outage; `warning`, `minor` → Degraded; `info`, `none` → Notice; missing → Outage |
| `labels.instance` (or `host`, `hostname`, `node`) | host |
| `labels.service`, else `job`, else `alertname` | service |

Every notification contains all alerts of the group; alerts that are already open are answered with
`duplicate`, so `repeat_interval` does not create new messages. Silenced and inhibited alerts are not sent.

## 4. Test

With the sample payloads, from any machine that reaches Notiflag:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-firing.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-resolved.json
# {"events":["cancelled"],"ok":true}
```

Through Alertmanager itself – push a test alert with `amtool`, then end it:

```bash
amtool alert add NotiflagTest audience=users instance=am-test severity=warning \
  --annotation='summary="Notiflag test"' --alertmanager.url=http://localhost:9093
# after group_wait the message appears; to resolve it, end the alert:
amtool alert add NotiflagTest audience=users instance=am-test severity=warning \
  --annotation='summary="Notiflag test"' --end="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --alertmanager.url=http://localhost:9093
```

The resolved notification follows within `group_interval`. Wait until the message has arrived before ending
the alert: an alert that resolves before its first notification is never sent at all.

## 5. Example rule

The cleanest way is to write the user-facing words into the Prometheus rule (`notiflag-rules.yml`).
For alerts you cannot change, a Notiflag rule translates them (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `*/HostDown` |
| Show this title | `{host} is not reachable` |
| And this message | `Services on this server are unavailable. We are working on it.` |
| Severity | As reported |

`*/HostDown` matches the subject `instance/service`, which is `db1:9100/HostDown` for an alert without
`service` and `job` labels, or use the title: `*is down*`.

## Troubleshooting

Alertmanager logs failed deliveries (`level=error … msg="Notify for alerts failed"`), and exposes
`alertmanager_notifications_failed_total{integration="webhook"}`.

| Symptom | Fix |
|---|---|
| `unexpected status code 401` | Token file contains more than the token (whole URL, `Bearer ` prefix), or the integration was deleted. |
| `unexpected status code 400` | Not Alertmanager's format – a template or proxy changed the body. |
| `x509: certificate signed by unknown authority` | Add `tls_config: { ca_file: /etc/ssl/certs/your-root-ca.pem }` under `http_config`, or install the CA system-wide. |
| `context deadline exceeded` | Notiflag not reachable; set `proxy_url` under `http_config` if needed. |
| Message appears, never resolves | `send_resolved` missing, or the alert was silenced before it resolved (silenced alerts send nothing). |
| Nothing arrives | Route does not match: `amtool config routes test --config.file=alertmanager.yml audience=users alertname=Test`. |
