# Notiflag for N-able N-central

When an N-central service fails on a device users depend on – the file server's disk, the mail server's
Exchange services, the firewall's connectivity – a message appears on users' desktops. When the service
returns to *Normal*, the message gets its green all-clear and disappears.

N-central's notification method **Third Party Integration – HTTP** posts an XML document; Notiflag
reads it as it is.

| File | Purpose |
|---|---|
| [`test-failed.xml`](test-failed.xml), [`test-normal.xml`](test-normal.xml) | Notifications in N-central's HTTP-XML layout, for testing with `curl` |

**Not tested in N-central.** The sample files follow the field names of N-central's notification
templates (`QualitativeNewState`, `DeviceName`, `AffectedService` …); Notiflag's parser is tested with
them. If a real notification does not behave as described, please send its XML (Troubleshooting).

**N-sight RMM** (the other N-able RMM) is covered in [other RMMs](../other-rmm/README.md).

## 1. Create the integration in Notiflag

One integration **per customer**, in that customer's Notiflag: *Monitoring → Add integration*, name
`N-central`, policy **Hold for approval**, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up N-central

### 2.1 A "user" for the customer's Notiflag

N-central sends notifications to users, so each Notiflag is a user with an HTTP notification method:

1. At the customer level: *Administration → User Management → Users → Add*, e.g. `Notiflag Example GmbH`
   (no login needed; any role with read access).
2. *Notification Method → Add*: type **Third Party Integration – HTTP**, name `Notiflag`,
   URL: the customer's webhook URL from step 1.
3. Keep the default HTTP-XML template, or make sure it contains at least `DeviceName`,
   `AffectedService`, `QualitativeNewState`, `CustomerName`, `QuantitativeNewState` and `TaskIdent`.

### 2.2 Notification profile

*Configuration → Monitoring → Notifications → Add*, at the customer level:

* Name `Notiflag`, the devices or device filter users depend on (e.g. a filter *Servers* or *Core network*).
* Trigger on the states **Failed** and, if wanted, **Warning**; tick **notify on return to normal**.
* Recipients: the user `Notiflag …` from 2.1.
* *Delay* a few minutes against short blips (or use Notiflag's delay).

## 3. How N-central maps to Notiflag

| N-central | Notiflag |
|---|---|
| `QualitativeNewState` *Failed* | problem, Outage |
| `QualitativeNewState` *Warning* | problem, Degraded |
| `QualitativeNewState` *Normal* | recovery: message resolved |
| *Misconfigured*, *No Data*, *Disconnected* … | ignored |
| `CustomerName` / `DeviceName` / `AffectedService` | key `ncentral-<customer>/<device>/<service>` |
| `DeviceName` / `AffectedService` | host / service; title `service (device)` |
| `QuantitativeNewState` – `TaskIdent` | message text without rule |

A change from *Warning* to *Failed* is answered with `duplicate` (one message per service).

## 4. Test

N-central's own *test* notification does not contain a state; Notiflag answers `{"events":[],"ok":true}`.

With the sample files:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: text/xml' --data-binary @test-failed.xml
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: text/xml' --data-binary @test-normal.xml
# {"events":["cancelled"],"ok":true}
```

End to end: a *Disk* service on a test device with a low threshold, notification profile as above.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `FS01/*` | `The file server has a problem` | `Network drives may be slow or unavailable. We are working on it.` | As reported |

The subject is `<device>/<service>`, e.g. `FS01/Disk Drive Space`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Nothing arrives | User not a recipient of the notification profile, profile not at the right customer level, or N-central cannot reach Notiflag (firewall, certificate). |
| `JSON body required` | The body was neither JSON nor XML – check the notification template. |
| `{"events":[],"ok":true}` for real notifications | The template lacks `QualitativeNewState`, or the state is one Notiflag ignores. Capture the XML (e.g. in your reverse proxy log) and send it in. |
| Message never resolves | *Notify on return to normal* is off. |
