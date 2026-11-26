---
id: appearance
title: Darstellung und Töne
description: Name, Farbe, Logo, Sprache, Position und Höhe der Leiste, Vollbild-Verhalten, Bezeichnungen und Töne einstellen.
lang: de
slug: darstellung
order: 10
---

# Darstellung und Töne

Alles, was Anwender sehen und hören, stellst du unter **Einstellungen** ein. Nach **Speichern** gilt es sofort
auf allen Clients – ohne Neuverteilung.

## Branding

| Einstellung | Standard | Hinweise |
|---|---|---|
| *Name* | Notiflag | Steht in der Weboberfläche (und beim Dienstleister im Hub). Höchstens 70 Zeichen |
| *Untertitel* | „Service status notifications“ | Zeile unter dem Namen in der Weboberfläche. Höchstens 140 Zeichen |
| *Farbe* | `#1f3a5f` | Farbe der Seitenleiste in der Weboberfläche |
| *Sprache* | Englisch | Sprache der Weboberfläche (siehe unten) |
| *Logo* | – | PNG, JPEG oder WebP, höchstens 1 MB. *Logo entfernen* setzt es zurück |

## Die Leiste

**Einstellungen → Leiste auf den Arbeitsplätzen**:

| Einstellung | Standard | Bereich | Hinweise |
|---|---|---|---|
| *Position* | Oben | Oben, Unten | |
| *Höhe (px)* | 44 | 24–160 | Normale Höhe, wird pro Monitor nach DPI skaliert |
| *Höhe bei neuer Meldung (px)* | 88 | Höhe – 300 | Eine neue Meldung erscheint zuerst größer … |
| *…für (s)* | 5 | | … und schrumpft nach dieser Zeit |
| *Entwarnung anzeigen für (s)* | 90 | 0–3600 | So lange bleibt die grüne Entwarnung sichtbar |
| *Auf jedem Monitor anzeigen* | an | | Aus = nur auf dem Hauptmonitor |
| *Über Vollbildfenstern sichtbar bleiben* | an | | Siehe [Windows-Client](windows-client.md#vollbild-und-remotedesktop) |
| *Wenn eine Remotedesktop-Sitzung einen Monitor ausfüllt* | Ausblenden | | Ausblenden, darüber anzeigen oder (experimentell) aus dem Vollbild nehmen |
| *Bezeichnungen vor dem Titel* | Outage, Degraded, Notice, Resolved, Silent alarm | | Das Etikett vor dem Titel pro Art, z. B. „Störung“, „Einschränkung“, „Hinweis“, „Erledigt“, „Stiller Alarm“ |

Die Vorschau oben auf der Seite **Meldungen** zeigt, wie die Leiste mit diesen Einstellungen aussieht.

Tipp für deutschsprachige Arbeitsplätze: Die Bezeichnungen sind ab Werk englisch. Trage einmal die deutschen
Begriffe ein.

## Töne

**Einstellungen → Töne**: Für jede Art – stiller Alarm, Störung, Einschränkung, Hinweis, Entwarnung – wählst du

- *Kein Ton* (Standard),
- einen Windows-Ton (`SystemHand`, `SystemExclamation`, `SystemAsterisk`, `SystemNotification`, `SystemDefault`,
  `Notification.Default`, `Notification.IM`, `Notification.Mail`, `Notification.Reminder`, `Notification.SMS`), oder
- *Eigener Ton (WAV)*: eine WAV-Datei bis 1 MB hochladen. *Anhören* spielt sie im Browser ab.

So klingt es an den Arbeitsplätzen:

- Ein Ton pro neuer Meldung, und zwar der der dringendsten neuen Art. Die Entwarnung einer Meldung, die gerade zu
  sehen war, spielt den Entwarnungston.
- Meldungen, die beim Start des Clients schon da sind, bleiben stumm.
- Windows-Töne folgen dem Soundschema und der Lautstärke des Computers – stummgeschaltet bleibt stumm.
- Eigene Töne lädt der Client einmal herunter und legt sie in `%LOCALAPPDATA%\Statusbar\sounds` ab.
- Der Computer, der einen stillen Alarm ausgelöst hat, spielt nichts.

## Sprache

Die Weboberfläche gibt es auf Deutsch und Englisch (*Einstellungen → Branding → Sprache*). Weitere Sprachen:

1. `server/locales/en.json` aus dem Repository kopieren und die Werte übersetzen (die Schlüssel bleiben).
2. Als `/data/locales/<code>.json` ins Volume legen, z. B. `fr.json`.
3. Unter *Sprache* den Code eintragen.

Die Texte **auf der Leiste** (Titel, Nachricht, Bezeichnungen) schreibst du selbst – sie sind unabhängig von der
Sprache der Oberfläche.
