---
id: appearance
title: Appearance and sounds
description: Set name, colour, logo, language, position and height of the bar, full-screen behaviour, labels and sounds.
lang: en
slug: appearance
order: 10
---

# Appearance and sounds

Everything users see and hear is set under **Settings**. After **Save** it applies to all clients immediately – no
redeployment.

## Branding

| Setting | Default | Notes |
|---|---|---|
| *Name* | Notiflag | Shown in the web UI (and to the service provider in the hub). At most 70 characters |
| *Subtitle* | "Service status notifications" | Line below the name in the web UI. At most 140 characters |
| *Colour* | `#1f3a5f` | Colour of the side rail in the web UI |
| *Language* | English | Language of the web UI (see below) |
| *Logo* | – | PNG, JPEG or WebP, at most 1 MB. *Remove logo* resets it |

## The bar

**Settings → Bar on the desktops**:

| Setting | Default | Range | Notes |
|---|---|---|---|
| *Position* | Top | Top, Bottom | |
| *Height (px)* | 44 | 24–160 | Normal height, scaled per monitor DPI |
| *Height when new (px)* | 88 | height – 300 | A new message first appears taller … |
| *…for (s)* | 5 | | … and shrinks after this time |
| *Show all-clear for (s)* | 90 | 0–3600 | How long the green all-clear stays |
| *Show on every monitor* | on | | Off = primary monitor only |
| *Stay visible above full-screen windows* | on | | See [Windows client](windows-client.md#full-screen-windows-and-remote-desktop) |
| *When a Remote Desktop session fills a monitor* | Hide | | Hide, show on top, or (experimental) take out of full screen |
| *Labels in front of the title* | Outage, Degraded, Notice, Resolved, Silent alarm | | The label in front of the title per kind |

The preview at the top of the **Messages** page shows the bar with these settings.

## Sounds

**Settings → Sounds**: for each kind – silent alarm, outage, degraded, notice, all-clear – choose

- *No sound* (default),
- a Windows sound (`SystemHand`, `SystemExclamation`, `SystemAsterisk`, `SystemNotification`, `SystemDefault`,
  `Notification.Default`, `Notification.IM`, `Notification.Mail`, `Notification.Reminder`, `Notification.SMS`), or
- *Own sound (WAV)*: upload a WAV file up to 1 MB. *Listen* plays it in the browser.

How it sounds on the desks:

- One sound per new message: the one of the most urgent new kind. The all-clear for a message that was on screen plays
  the all-clear sound.
- Messages that are already there when the client starts make no sound.
- Windows sounds follow the computer's sound scheme and volume – muted stays muted.
- Own sounds are downloaded once and kept in `%LOCALAPPDATA%\Statusbar\sounds`.
- The computer that raised a silent alarm plays nothing.

## Language

The web UI is available in English and German (*Settings → Branding → Language*). More languages:

1. Copy `server/locales/en.json` from the repository and translate the values (keep the keys).
2. Put it into the volume as `/data/locales/<code>.json`, e.g. `fr.json`.
3. Enter the code under *Language*.

The texts **on the bar** (title, message, labels) are written by you – they do not depend on the UI language.
