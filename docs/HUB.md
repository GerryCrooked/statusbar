# Service-provider hub

For IT service providers who run Statusbar for several customers.

```text
                    Provider                                  Customer A, B, C …
  ┌──────────────────────────────────────┐        ┌──────────────────────────────────┐
  │ Hub (HUB_ENABLED=yes)                │        │ Customer server                  │
  │  · customer overview                 │ ◄───── │  · own AD, branding, monitoring  │
  │  · messages to one/several/all       │  HTTPS │  · HUB_URL + HUB_TOKEN           │
  │  · central monitoring → customer     │  out-  │  · decides: show / approve / off │
  └──────────────────────────────────────┘  going └───────────────┬──────────────────┘
                                                                  │ long-poll
                                                          customer's desktops
```

Every customer keeps a complete server of their own; it keeps working if the hub is down. The hub is the same
software with one switch turned on.

## Set up

1. **Hub:** on the provider's server set `HUB_ENABLED=yes` in `.env` and restart. The web UI gets a *Customers* page.
2. **Customer:** *Customers → Add customer* shows two lines, `HUB_URL=…` and `HUB_TOKEN=…`. Put them into the
   `.env` of that customer's server and restart it. It connects out to the hub over HTTPS – no port forwarding,
   no VPN, nothing to open at the customer. The token is shown once; the hub stores only its hash.
3. The customer appears as *online* within seconds, with version, number of desktops and active messages.

## Send messages to customers

Create the message under *Messages* as usual and choose recipient **Customer**: a customer name, a pattern such as
`Praxis*`, or `*` for all customers. Several recipients can be combined.

* The message reaches the customer's desktops within about a second. It is shown to everyone at that customer.
* Messages addressed to customers are never shown on the hub's own desktops.
* *Resolve* on the hub sends the all-clear to every customer that received it.
* Central monitoring: a rule on the hub whose recipients are customers forwards matching alerts. Example: pattern
  `praxis-weber-*/*` → recipient *Customer: Praxis Dr. Weber*. Recovery resolves the message at the customer too.

## The customer stays in control

On the customer's server, *Settings → Messages from your service provider*:

| Setting | Effect |
|---|---|
| Show right away (default) | Hub messages appear on the desktops immediately |
| Hold for approval | Hub messages wait under *Waiting for approval* until a local admin publishes them |
| Do not accept | Hub messages are ignored |

The local admin can resolve a hub message at any time; it then stays resolved for that customer.

## What the hub sees

Per customer: online state and last contact, software version, number of desktops seen in the last 30 days, and the
titles and severity of active messages. Not sent: message texts, user names, computer names, directory data,
tokens or settings. The desktop count is the basis for licensing later on.

## Security

* Each customer has its own token; removing the customer on the hub locks its server out immediately.
* Hub messages are plain text; desktops render no links or markup.
* Whoever controls the hub can put text on the screens of every customer that accepts its messages. Protect the
  hub's admin access accordingly (HTTPS, network restriction, directory sign-in). Customers who want a second pair of
  eyes choose *Hold for approval*.
* The hub keeps its messages in its normal history.
