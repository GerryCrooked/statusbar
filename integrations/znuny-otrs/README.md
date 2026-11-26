# Statusbar for Znuny and OTRS

When IT gives a ticket for a known outage the highest priority (or puts it in an outage queue), a message
appears on every desktop, so users see that IT already knows. When the ticket is closed, the message
gets its green all-clear and disappears.

A small **Generic Agent module** does it: Znuny calls it on ticket events, it decides problem or
all-clear and sends one request to Statusbar.

| File | Purpose |
|---|---|
| [`Custom/Kernel/System/GenericAgent/Statusbar.pm`](Custom/Kernel/System/GenericAgent/Statusbar.pm) | The module |

Tested with Znuny 6.5 LTS: event-based Generic Agent job, a priority-5 ticket created and closed, a
normal ticket ignored, title with umlauts, quotes and a backslash. The module uses only APIs that
Znuny 6.x/7.x and OTRS 6 Community share.

(Znuny no longer offers *Execute custom command* in the Generic Agent; modules are the supported way.)

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Znuny`, policy **Hold for approval**, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Install the module

On the Znuny server, as root (paths for a standard installation in `/opt/otrs`; Znuny 7 may use
`/opt/znuny`):

```bash
install -d -o otrs -g www-data /opt/otrs/Custom/Kernel/System/GenericAgent
install -m 640 -o otrs -g www-data Statusbar.pm /opt/otrs/Custom/Kernel/System/GenericAgent/Statusbar.pm
echo 'https://status.example.com/api/hooks/sb_…' > /opt/otrs/var/statusbar-webhook-url
chown otrs:www-data /opt/otrs/var/statusbar-webhook-url && chmod 640 /opt/otrs/var/statusbar-webhook-url
su -s /bin/bash otrs -c '/opt/otrs/bin/otrs.Console.pl Maint::Cache::Delete'
systemctl reload apache2   # mod_perl loads modules at start
```

`Custom/` survives Znuny updates. Keeping the URL in a file keeps the token out of the Generic Agent
screen.

## 3. Generic Agent job

*Admin → Generic Agent → Add job*:

| Section | Setting |
|---|---|
| General | Job name `Statusbar`, Validity *valid* |
| Automatic execution | **none** (leave the schedule empty) |
| Event based execution | add the events **TicketCreate**, **TicketStateUpdate**, **TicketPriorityUpdate**, **TicketQueueUpdate**, **TicketTitleUpdate** |
| Select tickets | leave empty – the module decides which tickets count |
| Execute custom module | Module `Kernel::System::GenericAgent::Statusbar` |
| | Param 1: key `URLFile`, value `/opt/otrs/var/statusbar-webhook-url` |
| | Param 2: key `MinPriority`, value `5` |

Instead of (or in addition to) the priority:

| Parameter | Meaning |
|---|---|
| `MinPriority` | priority ID from which a ticket counts; default `5` (*5 very high*), unless `Queue` or `Type` is given |
| `Queue` | only tickets in this queue, full name, e.g. `IT::Outages` |
| `Type` | only tickets of this type, e.g. `Incident::Major` (ticket types must be enabled) |
| `URL` | the webhook URL itself instead of `URLFile` (then visible to every admin) |

The setting *Ticket::GenericAgentAllowCustomModuleExecution* must be on (the default).

## 4. How Znuny maps to Statusbar

| Znuny | Statusbar |
|---|---|
| ticket matches the criteria and its state type is not *closed*, *merged* or *removed* | problem: message opens |
| ticket closed, merged, removed – or no longer matches (priority lowered, moved out of the queue) | recovery: message resolved |
| ticket number | key `znuny-<number>` |
| title | title (and service, for rules) |
| queue | host |
| priority 5 / 4 / lower | Outage / Degraded / Notice |

The module is called for every ticket with one of the events; tickets that never matched are answered
with `ignored` and change nothing. Further events of an open matching ticket are answered with `duplicate`.

## 5. Test

Create a ticket with priority *5 very high*: Statusbar shows a draft. Close it: the draft is deleted
(`cancelled`) or the message resolved. Each call is logged by Znuny (*Admin → System Log*):
`Statusbar: znuny-2026112010000017 problem: {"events":["draft"],"ok":true}`.

## 6. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `IT::Outages/*` | `{title}` | `IT is working on it. No need to open a ticket.` | As reported |

The subject is `<queue>/<ticket title>`.

## Troubleshooting

| System log | Fix |
|---|---|
| `Use module (…) is not allowed by the system configuration` | Turn on *Ticket::GenericAgentAllowCustomModuleExecution*. |
| `Can't locate Kernel/System/GenericAgent/Statusbar.pm` | File not in `Custom/Kernel/System/GenericAgent/`, or Apache not reloaded. |
| `Statusbar: cannot read …` | URL file missing or not readable by the `otrs` user. |
| `Statusbar: ticket …: 401 Unauthorized` | URL incomplete or integration deleted. |
| `500 Can't connect` / `SSL connect attempt failed` | Network, or Znuny does not trust your certificate (install your root CA on the Znuny server). A proxy from *WebUserAgent::Proxy* is used. |
| Nothing in the log | Job not event-based, events missing, or job invalid. |

## Statusbar → Znuny

The other direction: Statusbar opens a Znuny ticket when a message is published or a silent alarm is
raised, and adds an internal article and closes it when the message is resolved. Overview and templates:
[Statusbar → ticket system](../../docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message).

Tested with Znuny 6.5 LTS in Docker: ticket in a queue with priority and customer user, test ticket,
silent alarm, internal article and state *closed successful*, title with umlauts, quotes and backslash, and
the Generic Agent job from step 3 configured for *every* ticket: Znuny called it before it had answered
Statusbar, and the call was recognised as Statusbar's own ticket.

1. Import the web service: *Admin → Web Services → Add web service → Import web service*, file
   [`Statusbar-Webservice.yml`](Statusbar-Webservice.yml), name **`Statusbar`**. It offers `TicketCreate`
   (`POST /Ticket`) and `TicketUpdate` (`PATCH /Ticket/:TicketID`) over REST. Console:
   `bin/otrs.Console.pl Admin::WebService::Add --name Statusbar --source-path Statusbar-Webservice.yml`
2. An agent with *rw* on the queue, and a customer user for the tickets (e.g. `statusbar`).
3. In Statusbar, *Settings → Ticket system*:

| Field | Value |
|---|---|
| Ticket system | Znuny / OTRS |
| Address | up to `/otrs`, e.g. `https://helpdesk.example.com/otrs` (Statusbar calls `…/nph-genericinterface.pl/Webservice/Statusbar/Ticket`) |
| User name / password | the agent's login and password (the GenericInterface takes them in the request body: use HTTPS) |
| Queue | e.g. `IT::Outages` |
| Customer user | e.g. `statusbar` |
| State when closing | empty = `closed successful` |
| Priority | empty = `4 high` / `3 normal` / `2 low` / `5 very high` (Outage / Degraded / Notice / Alarm) |

4. *Create test ticket*.

Tickets opened by Statusbar never come back as a second message, see [No loops](../../docs/INTEGRATIONS.md#no-loops-a-statusbar-ticket-never-becomes-a-second-message). The module from step 2 above also skips tickets whose first article contains
`statusbar-origin` – update `Statusbar.pm` if you installed an older copy.
