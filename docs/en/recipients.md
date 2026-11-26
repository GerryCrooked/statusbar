---
id: recipients
title: Recipients
description: Send messages to everyone, to computers by name pattern, to AD groups, OUs, users or notification groups.
lang: en
slug: recipients
order: 7
---

# Recipients

Without recipients a message goes to **everyone**. Otherwise every computer and every user that at least one of the
chosen recipients matches sees it.

## Kinds of recipients

| Kind | Example | Matches | Needs |
|---|---|---|---|
| **Computer** | `PC-RECEPTION-01` | exactly this computer; picked from the computers seen in the last 30 days | – |
| **Computer pattern** | `PC-ACC-*`, `TS0?` | computer names; `*` = any number of characters, `?` = exactly one, case-insensitive | – |
| **Notification group** | *Accounting* | everything the group contains | – |
| **Group** | `CN=Accounting,OU=Groups,DC=example,DC=local` | direct members of the AD group | Active Directory |
| **OU** | `OU=Site Ulm,DC=example,DC=local` | users in this OU and below | Active Directory |
| **User** | `anna` | exactly this sign-in name (`sAMAccountName`) | Active Directory |

In the form you choose the kind and type the start of the name; Notiflag suggests matching computers, groups, OUs,
users and notification groups. At most 50 recipients per message.

**Good to know:**

- Computer patterns compare the **computer name** the client reports – on a terminal server that is the name of the
  terminal server.
- AD groups count **direct** members only; nested groups are not expanded.
- Users, groups and OUs only work if the signed-in Windows user is found in the directory (see [Active
  Directory](active-directory.md)).

## Notification groups

A notification group bundles any recipients under one name – for example *Accounting* = OU Accounting + computers
`PC-ACC-*` + user `boss`.

**Settings → Notification groups → Create or change a group**: name, members (the same kinds as above), **Save**.
The group can then be chosen as recipient for every message, rule and integration.

Why groups?

- Membership is maintained in one place; rules and integrations stay unchanged.
- With a [hub](service-provider-hub.md) your service provider sees only the group **names** and can address them –
  who is in them is your decision.
- The [silent alarm](silent-alarm.md) typically goes to a group such as *Reception and facilities*.

## Where recipients are set

| Place | Effect |
|---|---|
| New message | Recipients of this message |
| Rule (monitoring) | Recipients of every message the rule matches |
| Integration → *Default recipients* | Recipients of this integration's messages without a matching rule |
| Silent alarm | Who sees the alarm |
| Service provider's hub | Everyone at your organisation or one of your notification groups |

Recipients decide **who is shown** a message – not who could read it. Confidential information does not belong in
messages (see [Security and privacy](security-and-privacy.md)).
