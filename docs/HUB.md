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

A local admin can resolve a hub message at any time; it then stays resolved here. Every hub message is marked
as published by the service provider.

The provider can address everyone at your organisation or one of your **notification groups**
(*Settings → Notification groups*). Who belongs to a group is decided here; the hub only knows its name.

## What is sent to the hub

Version of the server, its name, the computers that ran the client in the last 30 days (computer name, client
version, last contact), titles and severity of active messages, and the names of the notification groups.

Not sent: user names, message texts, directory data, settings, tokens.

## Protocol

For implementers of a hub. All calls are JSON over HTTPS.

`POST {HUB_URL}/api/hub/sync?rev=<n>&wait=<seconds>`, header `Authorization: Bearer <HUB_TOKEN>`, body:

```json
{ "version": "0.5.0", "name": "Acme", "clients": 42,
  "computers": [ { "computer": "pc-fin-01", "version": "0.5.31", "last_seen": 1794830000 } ],
  "incidents": [ { "title": "Printer floor 2", "severity": "warning", "source": "manual" } ],
  "teams": [ "Finance" ] }
```

Answer:

```json
{ "rev": 1794830123, "hub": "Provider name",
  "messages": [ { "id": "…", "title": "…", "message": "…", "severity": "error",
                  "status": "active", "teams": [ "Finance" ] } ] }
```

* `rev` is opaque. When the request's `rev` equals the current one, hold the request until something changes or
  `wait` seconds have passed (long-poll).
* `messages` lists everything currently relevant for this customer: active messages and recently resolved ones.
  A message that disappears from the list is resolved at the customer.
* `teams` empty means everyone at the customer.
* `401` means the token is not (or no longer) valid.

`scripts/fake-hub.py` is a minimal implementation used by the tests.
