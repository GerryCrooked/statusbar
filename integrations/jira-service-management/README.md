# Statusbar for Jira Service Management

When IT marks a Jira Service Management work item as a user-facing outage – the label `statusbar` – a
message appears on every desktop, so users see that IT already knows. When the work item is done, the
message gets its green all-clear and disappears.

Jira **Automation** does it with the action *Send web request*; nothing to install. Works the same in
Jira Software and Jira Work Management projects.

| File | Purpose |
|---|---|
| [`body-problem.json`](body-problem.json) | Web request body of rule 1 (outage) |
| [`body-resolved.json`](body-resolved.json) | Web request body of rule 2 (all-clear) |

**Not tested in a Jira site.** The bodies are the generic payload tested by
`scripts/test-integrations.sh`; the rule steps follow Atlassian's automation documentation.

Jira Cloud sends from Atlassian's cloud: Statusbar must be reachable from the internet over HTTPS with a
publicly trusted certificate, on port 443 (Jira Automation only allows a few ports, 443 and 8443 among
them). Jira Data Center sends from your own server.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Jira`, policy **Hold for approval**, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Jira

*Project settings → Automation → Create rule* (or *Settings → System → Global automation* for several
projects), twice.

### Rule 1: Statusbar – show outage

1. Trigger **Multiple work item events**: *Work item created*, *Work item updated*.
   (Older UIs: *Issue created* / *Issue updated*.)
2. Condition **Work item fields condition**: *Labels* **contains any of** `statusbar`.
3. Condition **Work item fields condition**: *Status category* **is not one of** `Done`.
4. Action **Send web request**:
   * Web request URL: the webhook URL from step 1 – tick **Hidden** for the URL
   * HTTP method `POST`
   * Web request body: **Custom data**, content of [`body-problem.json`](body-problem.json)
   * Headers: `Content-Type` = `application/json`
   * *Delay execution of subsequent rule actions until we've received a response*: off
5. Name `Statusbar – show outage`, *Turn on rule*.

```json
{
  "key": "jira-{{issue.key}}",
  "status": "problem",
  "host": "{{issue.project.name.jsonEncode}}",
  "service": "{{issue.summary.jsonEncode}}",
  "title": "{{issue.summary.jsonEncode}}",
  "severity": "{{issue.priority.name.jsonEncode}}"
}
```

`.jsonEncode` escapes quotes and backslashes in the inserted text; keep it on every text smart value.

### Rule 2: Statusbar – all-clear

1. Trigger **Work item transitioned**, *To status*: your done statuses (*Resolved*, *Closed*, *Done*).
2. Condition **Work item fields condition**: *Labels* **contains any of** `statusbar`.
3. Action **Send web request** as in rule 1, body [`body-resolved.json`](body-resolved.json):

```json
{
  "key": "jira-{{issue.key}}",
  "status": "ok",
  "host": "{{issue.project.name.jsonEncode}}",
  "service": "{{issue.summary.jsonEncode}}"
}
```

Want "major incident" instead of a label? Use JSM's *Major incident* field (*Major incident* **is**
*Major incident*) or *Priority* **is** *Highest* as the condition in both rules.

## 3. How Jira maps to Statusbar

| Jira | Statusbar |
|---|---|
| work item with label `statusbar`, not done | problem: message opens |
| work item transitioned to a done status | recovery: message resolved |
| `{{issue.key}}` | key `jira-ITSM-123` |
| summary | title (and service, for rules) |
| project name | host |
| priority *Highest*, *High* / *Medium* / *Low*, *Lowest* | Outage / Degraded / Notice |

Rule 1 also fires on later updates of the work item; Statusbar answers `duplicate`. A reopened work
item opens a new message.

## 4. Test

Create a work item *VPN not working* with the label `statusbar`. The rule's **Audit log**
(*Automation → the rule → Audit log*) shows the request and Statusbar's answer
`{"events":["draft"],"ok":true}`. Transition it to *Done*: rule 2 answers `cancelled` or `resolved`.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `IT Support/*VPN*` | `VPN is disrupted` | `Working from home is currently not possible. IT is on it.` | As reported |

The subject is `<project name>/<summary>`.

## Troubleshooting

| Audit log | Fix |
|---|---|
| `401` | URL incomplete or integration deleted. |
| `400 JSON body required` | A smart value without `.jsonEncode` inserted a quote, or *Custom data* is not selected. |
| connection / port error | Statusbar not reachable from the internet on 443/8443, or its certificate is not publicly trusted. |
| Rule does not run | Label condition misspelled, or rule limited to other projects. |
| Message stays | Rule 2 missing a done status, or the label was removed before the transition. *Resolve* it in Statusbar. |

## Statusbar → Jira Service Management

The other direction: Statusbar raises a request when a message is published or a silent alarm is raised,
and comments on it or moves it through a transition when the message is resolved. Overview and
templates: [Statusbar → ticket system](../../docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message).

**Not tested live** – built from Atlassian's REST API documentation (Jira Service Management Cloud,
`/rest/servicedeskapi`) and checked against a stand-in that answers like it. Try *Create test ticket* first.

| Field | Value |
|---|---|
| Ticket system | Jira Service Management |
| Address | `https://example.atlassian.net` (Data Center: your base URL) |
| User name / token | Cloud: the account's e-mail address and an [API token](https://id.atlassian.com/manage-profile/security/api-tokens). Data Center: a personal access token, user name empty |
| Service desk ID / request type ID | e.g. `1/10` – from `GET /rest/servicedeskapi/servicedesk` and `GET /rest/servicedeskapi/servicedesk/<id>/requesttype` |
| On behalf of | optional: account ID (Cloud) or user name of the reporter |
| Transition ID for closing | e.g. `761` – from `GET /rest/servicedeskapi/request/<key>/transition`; empty = only a comment |
| Priority | empty = not sent. Enter priority names (`Highest`, `High` …) only if the request type has the *Priority* field |

What Statusbar sends: `POST /rest/servicedeskapi/request` (`summary`, `description`); on resolve
`POST …/request/<key>/transition` with an internal comment, or `POST …/request/<key>/comment`
(`"public": false`).

Tickets opened by Statusbar never come back as a second message, see [No loops](../../docs/INTEGRATIONS.md#no-loops-a-statusbar-ticket-never-becomes-a-second-message). The key `jira-<issue key>` is known to Statusbar; you can also add the condition
*Description does not contain* `statusbar-origin` to the automation rules above.
