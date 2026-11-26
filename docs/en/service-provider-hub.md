---
id: service-provider
title: For IT service providers
description: Connect customer servers to the Notiflag hub, send messages to one, several or all customers, and what the customer controls.
lang: en
slug: service-provider-hub
order: 12
---

# For IT service providers

IT service providers run a central **hub** and connect their customers' Notiflag servers to it. The hub is a separate,
commercial product; the customer side is part of every Notiflag server.

```text
   Service provider                          Customer
 ┌──────────────────────┐            ┌─────────────────────────────────────┐
 │ Hub                  │  ◄───────  │ Notiflag server                     │
 │  · customer overview │   HTTPS,   │  · own directory, branding,         │
 │  · messages to one,  │  outgoing  │    monitoring, notification groups  │
 │    several or all    │    only    │  · HUB_URL + HUB_TOKEN              │
 │    customers         │            │  · decides: show / approve / off    │
 └──────────────────────┘            └──────────────────┬──────────────────┘
                                                        │
                                               customer's desktops
```

Every customer keeps a complete server of their own; it keeps working when the hub is unreachable.

## What the hub does

- **Customers**: create a customer, hand over two lines (`HUB_URL`, `HUB_TOKEN`). The customer server connects
  outbound – nothing has to be opened at the customer, no VPN, no port forwarding.
- **Overview** per customer: online, or offline since when, server version, computers with client version and last
  contact, active messages, notification groups; plus the total of computers across all customers (last 30 days).
- **Messages to customers**: recipient *Customer* with a name, a pattern (`Practice*`), `*` for all, or `Name/group`
  for one of a customer's notification groups (`*/Accounting` = that group at every customer). Per message the provider
  sees whether it is shown at the customer, waiting for approval, resolved or discarded – and by whom.
- **Central monitoring**: integrations and rules work in the hub like in any server; with recipient *Customer* a
  central monitoring feeds the customers' bars.
- **Silent alarms** of customers are visible to the provider with title and time and can be ended there.

The hub is itself a complete Notiflag server (with its own desktops, if wanted) and, like it, runs with Docker behind
an HTTPS reverse proxy. Image and hub license are available through [notiflag.de](https://notiflag.de/en/).

## Connect a customer

**At the service provider**, in the hub under **Customers → Add customer**: the customer's name, **Create**. The hub
shows the two lines for the customer once.

**At the customer**, in the Notiflag server's `.env`:

```dotenv
HUB_URL=https://hub.example-provider.com
HUB_TOKEN=sbh_…
```

```bash
docker compose up -d
```

*Settings* at the customer shows whether the connection is up and when the hub was last reached; the hub lists the
customer as *Online*. The customer server must reach `https://<hub>/api/hub/sync`.

To remove a customer: *Delete* in the hub. Its server can then no longer connect.

## What the customer controls

*Settings → Messages from your service provider*:

| Setting | Effect |
|---|---|
| *Show right away* (default) | Messages from the hub appear immediately |
| *Hold for approval* | They wait under *Waiting for approval* until a local admin publishes them |
| *Do not accept* | They are ignored |

A local admin can resolve a hub message at any time; it then stays resolved at the customer. A waiting message can be
discarded; it does not come back. Every hub message is marked as published by the service provider.

The provider can address everyone at the customer or one of the customer's **notification groups**. Who belongs to a
group is decided by the customer; the hub only knows its name.

## What is sent to the hub

- version and name of the server
- computers that ran the client in the last 30 days: computer name, client version, last contact
- titles and severity of active messages
- names of the notification groups
- the setting for messages from the provider and, for each of the provider's messages, its state at the customer with
  the name of the local admin who approved, resolved or discarded it
- for a silent alarm also its id and time; its title usually contains user and computer name

Not sent: user names (apart from alarm titles as configured), message texts, directory data, settings, tokens.

## License

Customer servers connected to a hub need no license of their own – whatever the number of PCs. The hub needs a hub
license (`LICENSE_KEY` in the hub). Partner terms on request through [notiflag.de](https://notiflag.de/en/). See
[License](license.md).

## Protocol

For developers of their own hub the protocol is documented openly:
[docs/HUB.md in the repository](https://github.com/GerryCrooked/statusbar/blob/main/docs/HUB.md#protocol).
