# Connecting to a service provider

How to connect a Notiflag server to a service provider's hub, what the customer controls and what is sent to the hub:

* English: [For IT service providers](en/service-provider-hub.md)
* Deutsch: [Für IT-Dienstleister](de/dienstleister-hub.md)

This page keeps the protocol for implementers of a hub.

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
