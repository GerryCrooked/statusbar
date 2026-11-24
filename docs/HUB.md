# Connecting to a service provider

IT service providers can run a central **hub** (a separate, commercial product) and connect the Statusbar
servers of their customers to it. This page describes the customer side, which is part of Statusbar.

```text
   Service provider                         Customer
 ┌──────────────────────┐            ┌───────────────────────────────────┐
 │ Hub                  │  ◄───────  │ Statusbar server                  │
 │  · customer overview │   HTTPS,   │  · own directory, branding,       │
 │  · messages to one,  │   outgoing │    monitoring, notification groups│
 │    several or all    │   only     │  · HUB_URL + HUB_TOKEN            │
 └──────────────────────┘            │  · decides: show / approve / off  │
                                     └────────────────┬──────────────────┘
                                                      │
                                              customer's desktops
```

The customer keeps a complete server of their own; it keeps working when the hub is unreachable.

## Connect

The service provider gives you two values. Put them into the server's `.env` and restart it:

```dotenv
HUB_URL=https://hub.example-provider.com
HUB_TOKEN=sbh_…
```

The server opens one outgoing HTTPS connection to the hub. No port forwarding, VPN or inbound rule is needed.
*Settings* shows whether the connection is up and when the hub was last reached.

## Stay in control

*Settings → Messages from your service provider*:

| Setting | Effect |
|---|---|
| Show right away (default) | Messages from the hub appear on the desktops immediately |
| Hold for approval | They wait under *Waiting for approval* until a local admin publishes them |
| Do not accept | They are ignored |

A local admin can resolve a hub message at any time; it then stays resolved here. A message waiting for
approval can be discarded; it does not come back, even while the provider keeps it active. Every hub message
is marked as published by the service provider, and the provider sees per message whether it is waiting for
approval, shown, resolved or discarded here, and by whom.

The provider can address everyone at your organisation or one of your **notification groups**
(*Settings → Notification groups*). Who belongs to a group is decided here; the hub only knows its name.

## What is sent to the hub

Version of the server, its name, the computers that ran the client in the last 30 days (computer name, client
version, last contact), titles and severity of active messages, the names of the notification groups, the
setting for messages from the provider, and for each of the provider's messages its state here and the name of
the local admin who approved, resolved or discarded it.

For a silent alarm also its id and when it was raised, so the provider can end it. Its title is the configured
alarm title, which usually contains the user and computer name.

Not sent: user names (apart from alarm titles as configured), message texts, directory data, settings, tokens.

## Protocol

For implementers of a hub. All calls are JSON over HTTPS.

`POST {HUB_URL}/api/hub/sync?rev=<n>&wait=<seconds>`, header `Authorization: Bearer <HUB_TOKEN>`, body:

```json
{ "version": "0.5.0", "name": "Acme", "clients": 42,
  "computers": [ { "computer": "pc-fin-01", "version": "0.5.31", "last_seen": 1794830000 } ],
  "incidents": [ { "title": "Printer floor 2", "severity": "warning", "source": "manual" },
                 { "title": "Silent alarm from anna at PC-07", "severity": "alarm", "source": "alarm",
                   "id": "6f1c…", "created_at": 1794830100 } ],
  "teams": [ "Finance" ],
  "accepts": "approve",
  "states": [ { "id": "…", "state": "discarded", "by": "admin", "at": 1794830200 } ] }
```

`accepts` is `publish`, `approve` or `off`. `states` covers the provider's messages known here (newest first, at
most 200): `pending` (waiting for approval), `shown`, `resolved` or `discarded`; `by` is empty when nobody acted
locally (for example a message resolved by the provider). A server reports on every call; when something the
report covers changes locally (a message approved, discarded or resolved, the setting, the groups) it drops the
waiting long-poll and calls again at once. A hub must therefore store the report before it starts to wait, and
must tolerate requests that are closed while waiting.

Answer:

```json
{ "rev": 1794830123, "hub": "Provider name",
  "messages": [ { "id": "…", "title": "…", "message": "…", "severity": "error",
                  "status": "active", "teams": [ "Finance" ] } ],
  "resolve": [ "6f1c…" ] }
```

* `rev` is opaque. When the request's `rev` equals the current one, hold the request until something changes or
  `wait` seconds have passed (long-poll).
* `messages` lists everything currently relevant for this customer: active messages and recently resolved ones.
  A message that disappears from the list is resolved at the customer.
* `teams` empty means everyone at the customer.
* `resolve` (optional) ends silent alarms at the customer: ids from `incidents` with source `alarm`. Keep sending
  an id until it is gone from the report; repeats are harmless, ids of anything else are ignored. This works
  whatever the customer chose for messages from the provider. Servers before this version ignore the field.
* `401` means the token is not (or no longer) valid.

`scripts/fake-hub.py` is a minimal implementation used by the tests.
