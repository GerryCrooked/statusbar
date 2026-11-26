---
id: silent-alarm
title: Silent alarm
description: Call for help quietly with a key combination – set up, test, limits and privacy.
lang: en
slug: silent-alarm
order: 9
---

# Silent alarm

A user presses a key combination on their computer, for example `Ctrl+Shift+F12`. The chosen recipients immediately see
a violet message such as "Silent alarm from anna at PC-RECEPTION-01". **Nothing** appears on the computer where it was
pressed – no confirmation, no sound, not even in its own bar.

Meant for reception desks, counters, practices, law firms and anyone who may need help without drawing attention.

> **Not an emergency call system.** The silent alarm depends on the network, the server and someone looking at a
> screen. There is no acknowledgement towards the person who pressed the keys. Keep your regular emergency
> procedures and agree what an alarm means and who goes to check.

## Switch it on

**Settings → Silent alarm**:

| Setting | Default | Notes |
|---|---|---|
| *Allow the silent alarm on all computers* | off | Enables the key combination on all clients; off = it is removed again within seconds |
| *Key combination* | `Ctrl+Shift+F12` | Ctrl, Alt, Shift or Win plus a function key, or two of them plus a character (`Ctrl+Shift+Ü`) |
| *Title* | "Silent alarm from {user} at {computer}" | `{user}` and `{computer}` are replaced |
| *Message* | "Please check on them now." | What recipients should do |
| *Recipients* | everyone | Best a [notification group](recipients.md#notification-groups) such as *Reception and facilities* |
| *Ignore repeats for (s)* | 300 | After an alarm ends, further presses from the same session are ignored this long |
| *End after (min, 0 = when resolved)* | 0 | 0 = the alarm stays until someone resolves it |

Under *Sounds* you can also choose a sound, and under *Labels in front of the title* a label, for the silent alarm.

**Choosing the key combination:** it must contain Ctrl, Alt or Win, and two modifiers with a character key – so normal
typing and shortcuts such as Ctrl+C keep working. Pick one nobody presses by accident and no other program uses.
Characters such as `Ü` exist only on matching keyboard layouts.

## What recipients see

- A **violet** message with its own label.
- It stays visible: with other messages at the same time the alarm keeps the left part of the bar (40 %), the others
  rotate on the right.
- It also shows over full-screen windows and over a Remote Desktop session in mode *Hide*.
- Users cannot dismiss it. It ends when an admin resolves it under **Messages**, the service provider ends it in the
  hub, or the configured time runs out.

## Test

1. Switch it on, recipients = a test group with your own PC (not the one where you press the keys).
2. Press the combination on a second PC. Nothing appears there.
3. Your PC shows the violet message; under **Messages → Active** it is listed with origin *silent alarm*.
4. **Resolve**.

If the combination does not work on a PC, the reason is in `%LOCALAPPDATA%\Statusbar\client.log` (for example: already
used by another program, or the character is missing from the keyboard layout).

## Limits against false alarms and floods

- While an alarm from a session is open, pressing again there changes nothing.
- The client ignores a second press within 10 seconds.
- After an alarm ends, the server ignores further presses from the same session for the configured cooldown (default
  5 minutes).
- At most 5 alarms per 10 minutes per server; further ones are refused and logged.
- If the server is unreachable, the client retries for about a minute.

## Special cases

- **Remote Desktop in full screen:** keys go to the remote session. If the client is installed on the session host, it
  raises the alarm there – with the session host's name as computer.
- **Terminal servers:** "nothing shown on the raising computer" applies to every session on that server.
- **Older clients** show alarms in the outage colour and let them rotate like other messages.
- **Service provider with a hub:** the provider sees active alarms with title, id and time and can end them –
  regardless of the setting for messages from the provider.

## Privacy and records

- Every alarm is a message with origin `alarm:<user>@<computer>` and is kept like other messages for `RETENTION_DAYS`.
- Every accepted, repeated, limited or refused press is written to the server log with user, computer and sender
  address (`X-Forwarded-For` from the reverse proxy).
- User and computer names are reported by the client itself; they are not cryptographically verified. The server only
  accepts computers that have connected in the last 30 days.
- The title usually contains the user name. Inform users before switching the silent alarm on and, where applicable,
  involve your data protection officer and works council.

More on the security model: [Security and privacy](security-and-privacy.md).
