# Statusbar for Zammad

When IT tags a Zammad ticket with `statusbar` – say "E-mail is down" – a message appears on every
desktop: users see that IT already knows and don't open twenty more tickets. When the ticket is closed,
the message gets its green all-clear and disappears.

Zammad does it with two **webhooks** and two **triggers**; nothing to install.

| File | Purpose |
|---|---|
| [`payload-problem.json`](payload-problem.json) | Custom payload of the webhook *Statusbar problem* |
| [`payload-resolved.json`](payload-resolved.json) | Custom payload of the webhook *Statusbar resolved* |

Tested with Zammad 7.2: ticket created with the tag, closed, including a title with quotes and a
backslash (Zammad escapes inserted values correctly).

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Zammad`, policy **Hold for approval** (IT checks the wording before
users see it) or *Publish as received* if your agents write ticket titles for users. Delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once. You need the
token part (`sb_…`) in step 2.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Zammad

### 2.1 Two webhooks

*Settings (gear) → Manage → Webhook → New Webhook*, twice:

| Field | Webhook 1 | Webhook 2 |
|---|---|---|
| Name | `Statusbar problem` | `Statusbar resolved` |
| Endpoint | `https://status.example.com/api/hooks` | the same |
| Request method | `POST` | `POST` |
| Authentication | **Bearer Token**: `sb_…` | the same |
| HMAC SHA1 Signature Token | empty | empty |
| SSL verification | yes | yes |
| Custom Payload | on, content of [`payload-problem.json`](payload-problem.json) | on, content of [`payload-resolved.json`](payload-resolved.json) |

```json
{
  "key": "zammad-#{ticket.number}",
  "status": "problem",
  "host": "#{ticket.group.name}",
  "service": "#{ticket.title}",
  "title": "#{ticket.title}"
}
```

```json
{
  "key": "zammad-#{ticket.number}",
  "status": "ok",
  "host": "#{ticket.group.name}",
  "service": "#{ticket.title}"
}
```

### 2.2 Two triggers

*Manage → Trigger → New Trigger*, twice:

| Field | Trigger 1 | Trigger 2 |
|---|---|---|
| Name | `Statusbar: show outage` | `Statusbar: all-clear` |
| Activated by | *Action* | *Action* |
| Action execution | **Selective** | **Selective** |
| Conditions | *Tags* **contains one** `statusbar` **and** *State* **is** `new`, `open`, `pending reminder`, `pending close` | *Tags* **contains one** `statusbar` **and** *State* **is** `closed`, `merged` |
| Execute changes on objects | *Webhook* → `Statusbar problem` | *Webhook* → `Statusbar resolved` |

*Selective* means the trigger fires when one of its conditions changes – adding the tag, closing the
ticket – and not on every reply.

## 3. How Zammad maps to Statusbar

| Zammad | Statusbar |
|---|---|
| ticket gets tag `statusbar` while open | problem: message opens |
| ticket with the tag is closed or merged | recovery: message resolved |
| `#{ticket.number}` | key `zammad-<number>` |
| ticket title | title (and service, for rules) |
| group | host |

Further updates of the open ticket are answered with `duplicate`. Removing the tag does not take the
message down – close the ticket or *Resolve* the message in Statusbar. A reopened ticket opens a new message.

Optional fields you can add to the payload: `"message": "#{ticket.note}"` for a ticket field you fill
for users, or `"severity": "warning"` (default Outage).

## 4. Test

1. Create a ticket *E-mail is down*, add the tag `statusbar`.
2. Statusbar shows it under *Messages* (draft or published). The webhook's request log in Zammad shows
   `200` and `{"events":["draft"],"ok":true}`.
3. Close the ticket: the message is resolved (or the draft deleted: `cancelled`).

Zammad logs every webhook call: *Manage → Webhook →* the webhook *→ View Logs*.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `*mail*` | `E-mail is disrupted` | `IT is working on it. No need to open a ticket.` | Degraded |

The subject is `<group>/<ticket title>`; the rule matches the title too.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Webhook log: `401` | Bearer token wrong (enter only `sb_…`) or integration deleted. |
| Webhook log: SSL error | Zammad does not trust your certificate: add your root CA to the Zammad host/container (Zammad also offers *Manage → SSL Certificates*). |
| Nothing happens | Trigger set to *Always* with other conditions, tag misspelled, or the state list in trigger 1 does not contain the ticket's state. |
| Message stays after closing | Trigger 2 missing or inactive, or the tag was removed before closing. |
