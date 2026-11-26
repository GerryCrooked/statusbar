# Statusbar for TOPdesk

When an operator marks a TOPdesk incident as a **major incident**, a message appears on every desktop,
so users see that IT already knows. When the major incident is completed or closed, the message gets its
green all-clear and disappears.

TOPdesk's **Action Management** (automated actions with an HTTP request step) does it; nothing to install.

**Not tested in TOPdesk** – TOPdesk is SaaS and needs a customer environment. The request is the
generic payload tested by `scripts/test-integrations.sh`. Menu names differ slightly between TOPdesk
versions and languages; the placeholders are inserted with the editor's placeholder picker.

TOPdesk SaaS sends from TOPdesk's cloud: Statusbar must be reachable from the internet over HTTPS with a
publicly trusted certificate.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `TOPdesk`, policy **Hold for approval**, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up TOPdesk

*Modules → Supporting files → Action Management* (Dutch: *Actiebeheer*), two automated actions on
**Incidents**:

### Action 1: Statusbar – major incident

* **Trigger**: incident changed (or created) **and** *Major incident* is *Yes* **and** *Closed* is *No*.
* **Action sequence**, step **HTTP request**:
  * Method `POST`, URL: the webhook URL from step 1
  * Header `Content-Type: application/json`
  * **Escape values inserted into body**: on (the default) – TOPdesk then escapes quotes in the
    inserted texts
  * Body (insert *incident number*, *brief description* and *operator group* with the placeholder picker):
    ```json
    {
      "key": "topdesk-<incident number>",
      "status": "problem",
      "host": "<operator group>",
      "service": "<brief description>",
      "title": "<brief description>"
    }
    ```

### Action 2: Statusbar – all-clear

* **Trigger**: incident changed **and** *Major incident* is *Yes* **and** (*Completed* is *Yes* **or**
  *Closed* is *Yes*).
* Same HTTP request, body:
  ```json
  {
    "key": "topdesk-<incident number>",
    "status": "ok",
    "host": "<operator group>",
    "service": "<brief description>"
  }
  ```

Further changes of the open major incident are answered with `duplicate`.

## 3. How TOPdesk maps to Statusbar

| TOPdesk | Statusbar |
|---|---|
| major incident, not closed | problem: message opens |
| major incident completed or closed | recovery: message resolved |
| incident number | key `topdesk-<number>` |
| brief description | title (and service, for rules) |
| operator group | host |

Add `"severity": "warning"` to the first body for *Degraded* instead of *Outage*.

## 4. Test

Mark a test incident as major incident; the action's execution log shows the HTTP answer
`{"events":["draft"],"ok":true}`. Complete it: `cancelled` or `resolved`.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `*/*VPN*` | `VPN is disrupted` | `IT is working on it. No need to call.` | As reported |

## Troubleshooting

| Symptom | Fix |
|---|---|
| `401` | URL incomplete or integration deleted. |
| `400 JSON body required` | *Escape values inserted into body* is off, or a placeholder sits outside the quotes. |
| No request at all | Trigger conditions do not match (major incident flag, module *Incidents*), or the action is inactive. |
| Message stays | Action 2 missing, or *Major incident* was switched off before closing – *Resolve* it in Statusbar. |

## Statusbar → TOPdesk

The other direction: Statusbar registers an incident when a message is published or a silent alarm is
raised, and adds an action and closes it when the message is resolved. Overview and templates:
[Statusbar → ticket system](../../docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message).

**Not tested live** – built from TOPdesk's REST API documentation (`/tas/api/incidents`) and checked
against a stand-in that answers like it. Try *Create test ticket* first.

| Field | Value |
|---|---|
| Ticket system | TOPdesk |
| Address | `https://example.topdesk.net` |
| User name / password | an operator's login name and an **application password** (*My settings → Application passwords*); the operator needs the permission to use the REST API and to create incidents |
| Category | optional, the category's name |
| Caller name | optional, default `Statusbar` (an unregistered caller) |
| Processing status when closing | optional, the status' name (e.g. `Closed`) |
| Priority | empty = not sent; priority names differ per installation |

What Statusbar sends: `POST /tas/api/incidents` with `caller.dynamicName`, `briefDescription` (80
characters), `request`, `externalNumber` = `statusbar-origin:<message id>`; on resolve
`PATCH /tas/api/incidents/id/<id>` with an action hidden from the caller, `completed` and `closed`.

Tickets opened by Statusbar never come back as a second message, see [No loops](../../docs/INTEGRATIONS.md#no-loops-a-statusbar-ticket-never-becomes-a-second-message). The automated actions above only react to major incidents, which Statusbar does not
set, and the key `topdesk-<number>` is known to Statusbar.
