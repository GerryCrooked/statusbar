# Statusbar for Microsoft 365 service health

> **Status: in development.** This integration is being tested against a real Microsoft 365 tenant. Settings and
> payload handling may still change; feedback and captured payloads are welcome.

When Microsoft has an incident that affects your tenant – Exchange Online not delivering mail, Teams
meetings failing, SharePoint not opening documents – a message appears on your users' desktops. When
Microsoft marks the incident resolved, the message gets its green all-clear and disappears. Users stop
calling the helpdesk for problems nobody in the company can fix.

Unlike the other integrations, nothing calls Statusbar here: **the Statusbar server asks Microsoft Graph**
every two minutes for the open service health issues of your tenant. You only register an app in
Microsoft Entra ID and put three values into `.env`.

| File | Purpose |
|---|---|
| [`test-issues.json`](test-issues.json) | Example Graph answer, used by the server's tests |

Tested against a stand-in for the Graph and sign-in endpoints (wrong secret, missing integration, two
incidents opening, one resolving, Graph unreachable) and with the server's unit tests. **Not yet tested
against a real tenant** – the requests follow Microsoft's documented Graph API.

```
Statusbar ──client credentials──► login.microsoftonline.com ──token──┐
          ──GET /admin/serviceAnnouncement/issues?$filter=isResolved eq false──► graph.microsoft.com
          ◄── open issues ── new issue: message opens · issue gone: all-clear · Graph down: nothing changes
```

## 1. Create the integration in Statusbar

*Monitoring → Add integration*:

* **Name**: `Microsoft 365` (exactly this, or set `M365_INTEGRATION` in step 3)
* **Alerts without a matching rule**: **Hold for approval** to start with. Microsoft lists many small
  incidents ("some users may…"); approve the relevant ones, and add rules (step 5) for the services
  that matter so those are published at once.
* **Wait before showing**: `0` – Microsoft posts incidents only after its own analysis.
* **Default recipients**: empty for everyone.

You do not need the webhook URL; the token is not used.

## 2. Register an app in Microsoft Entra ID

You need a Global Administrator or Privileged Role Administrator for the consent.

1. [Microsoft Entra admin center](https://entra.microsoft.com) → *Identity → Applications →
   App registrations → New registration*.
   * Name: `Statusbar service health`
   * Supported account types: *Accounts in this organizational directory only*
   * Redirect URI: none → *Register*.
2. On the app's *Overview*, copy **Application (client) ID** and **Directory (tenant) ID**.
3. *API permissions → Add a permission → Microsoft Graph → Application permissions* →
   **ServiceHealth.Read.All** → *Add permissions*, then **Grant admin consent for …**. The status
   column must show a green tick.
   That permission can only read service health; it gives no access to mail, files or users.
4. *Certificates & secrets → Client secrets → New client secret*: description `Statusbar`, expiry e.g.
   24 months → *Add*. Copy the **Value** right away (not the *Secret ID*); it is shown only once.
   Put a reminder in your calendar before it expires.

## 3. Configure the server

In `.env` next to `compose.yaml`:

```bash
M365_TENANT_ID=<Directory (tenant) ID>
M365_CLIENT_ID=<Application (client) ID>
M365_CLIENT_SECRET=<secret value>
```

```bash
docker compose up -d
docker compose logs statusbar | grep -i "microsoft 365"
# INFO … Microsoft 365 service health enabled integration=Microsoft 365 every=120
```

| Variable | Default | Meaning |
|---|---|---|
| `M365_TENANT_ID`, `M365_CLIENT_ID` | – | from step 2; without `M365_TENANT_ID` the feature is off |
| `M365_CLIENT_SECRET` or `M365_CLIENT_SECRET_FILE` | – | the secret, or a file containing it (Docker secret) |
| `M365_INTEGRATION` | `Microsoft 365` | name of the integration from step 1 |
| `M365_SERVICES` | all | comma-separated services, as named in the admin center, e.g. `Exchange Online,Microsoft Teams,SharePoint Online,OneDrive for Business` |
| `M365_CLASSIFICATIONS` | `incident` | `incident,advisory` also shows advisories (usually minor, rarely worth a message) |
| `M365_POLL_SECONDS` | `120` | minimum 60 |
| `M365_LOGIN_URL`, `M365_GRAPH_URL` | global cloud | national clouds, e.g. `https://login.microsoftonline.us` and `https://graph.microsoft.us` |

The server needs outbound HTTPS to `login.microsoftonline.com` and `graph.microsoft.com`. Behind a proxy,
set `HTTPS_PROXY=http://proxy.example.local:3128` in `.env`.

## 4. How Microsoft 365 maps to Statusbar

| Graph `serviceHealthIssue` | Statusbar |
|---|---|
| open issue (`isResolved` = false) of a selected service and classification | problem: message opens |
| issue resolved, or no longer selected | recovery: message resolved |
| `status` = `falsePositive` | ignored (and resolved if it was open) |
| `id` (e.g. `EX1234567`) | key `m365-EX1234567` |
| `title` | title |
| `impactDescription` | message text |
| `service` (e.g. `Exchange Online`) | service; host is always `Microsoft 365` |
| `status` = `serviceInterruption` | Outage |
| other incidents (`serviceDegradation`, `investigating`, `restoringService` …) | Degraded |
| advisories | Notice |

Updates Microsoft posts on an open issue do not create new messages. If Graph cannot be reached or the
sign-in fails, nothing is resolved – the messages stay until the next successful check.

## 5. Example rules

In Statusbar (*Monitoring → Add rule*). The subject is `Microsoft 365/<service>`:

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `Microsoft 365/Exchange Online` | `E-mail is disrupted (Microsoft)` | `{title}. Microsoft is working on it – no need to call the helpdesk.` | As reported |
| `Microsoft 365/Microsoft Teams` | `Teams is disrupted (Microsoft)` | `{title}. Please use the phone for urgent calls.` | As reported |
| `m365-EX1234567` | … | … | a single incident, by its ID |

With *Hold for approval* as policy, everything else waits for you under *Messages → Waiting for approval*.

## 6. Check that it works

* **Monitoring** page: the integration `Microsoft 365` shows *Microsoft 365 checked … , n open issue(s)* –
  or the error from Microsoft in red.
* The same as JSON, including the open issues:
  ```bash
  curl -s -H "Authorization: Bearer $ADMIN_TOKEN" https://status.example.com/api/admin/m365
  ```
* Compare with *Microsoft 365 admin center → Health → Service health*. Statusbar sees the same list
  (only issues that affect your tenant).

There is no way to make Microsoft open a test incident. To see a message end to end, set
`M365_CLASSIFICATIONS=incident,advisory` for a moment – there is almost always an open advisory – and
restart; drafts appear under *Waiting for approval*. Discard them and set it back.

## Troubleshooting

The error is shown on the Monitoring page and logged once when it first occurs:
`docker compose logs statusbar | grep -i "microsoft 365"`.

| Error | Fix |
|---|---|
| `AADSTS7000215: Invalid client secret provided` | The *Secret ID* was copied instead of the *Value*, or the secret was deleted. Create a new secret. |
| `AADSTS7000222: The provided client secret keys … are expired` | Create a new secret, update `.env`, `docker compose up -d`. |
| `AADSTS700016: Application … was not found in the directory` | Client ID and tenant ID do not belong together, or the app was deleted. |
| `AADSTS90002: Tenant … not found` | Wrong tenant ID. |
| `Graph answered 403 Forbidden` | `ServiceHealth.Read.All` is missing, is a *delegated* instead of an *application* permission, or admin consent was not granted. |
| `no integration named "Microsoft 365"` | Create the integration (step 1) or set `M365_INTEGRATION` to its name. |
| `error sending request` | No outbound HTTPS from the server, DNS, or a proxy (`HTTPS_PROXY`). |
| Server does not start: `M365_CLIENT_ID is missing` | `M365_TENANT_ID` is set, but not all three values. |
| Too many messages | Restrict `M365_SERVICES`, keep `M365_CLASSIFICATIONS=incident`, use *Hold for approval*. |
