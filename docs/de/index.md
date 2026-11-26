---
id: overview
title: Was ist Notiflag?
description: Notiflag zeigt allen Arbeitsplätzen eine Statusleiste, wenn etwas gestört, eingeschränkt oder geplant ist – automatisch aus dem Monitoring oder per Klick.
lang: de
slug: ueberblick
order: 1
---

# Was ist Notiflag?

Notiflag legt eine schmale farbige Leiste an den Rand der Windows-Desktops, sobald etwas gestört,
eingeschränkt oder geplant ist – und nimmt sie mit einer grünen Entwarnung wieder weg, wenn es vorbei ist.
Die Anwender wissen Bescheid, bevor das Telefon klingelt.

Meldungen kommen automatisch aus dem Monitoring (Zabbix, Checkmk, PRTG, Microsoft 365 und über 20 weitere
Systeme) oder per Klick von einem Admin.

## Die Bausteine

```text
 Monitoring, Ticketsystem, Skript          Admin im Browser
          │  Webhook                            │
          ▼                                     ▼
 ┌──────────────────────────────────────────────────────┐
 │ Notiflag-Server (Docker, eine Datei SQLite)          │
 │  Meldungen · Regeln · Empfänger · Branding · Töne    │
 └───────────────────────────┬──────────────────────────┘
                             │ HTTPS, Long-Poll (~1 Sekunde)
                             ▼
          Windows-Client auf jedem Arbeitsplatz
          (zeigt die Leiste, optional: stiller Alarm)
```

| Baustein | Was er tut | Wo er läuft |
|---|---|---|
| **Server** | Nimmt Meldungen an, entscheidet, wer sie sieht, liefert sie aus. Weboberfläche für Admins. | Linux mit Docker, bei dir im Haus oder in deiner Cloud |
| **Windows-Client** | Zeigt die Leiste am oberen oder unteren Bildschirmrand, spielt Töne, löst den stillen Alarm aus. | Jeder Windows-10/11-Arbeitsplatz und jeder Terminalserver |
| **Hub** (optional, für IT-Dienstleister) | Verbindet die Server vieler Kunden: Übersicht und Meldungen an einen, mehrere oder alle Kunden. | Beim Dienstleister |

## Was Notiflag kann

- **Selbst gehostet und klein.** Ein Server mit SQLite, ein Windows-Programm. Keine Cloud, keine Laufzeitumgebung
  auf den Arbeitsplätzen, kein Dienst mit Adminrechten.
- **Sofort sichtbar.** Eine neue Meldung ist in etwa einer Sekunde auf allen Bildschirmen.
- **Aus dem Monitoring.** Ein Problem öffnet eine Meldung, die Erholung entwarnt sie. Anleitungen für
  Monitoring-Systeme, RMM-Werkzeuge, Ticketsysteme und Microsoft 365: [Monitoring anbinden](monitoring.md).
- **Verständlich.** Regeln machen aus `mail01/SMTP CRITICAL` die Meldung „E-Mail ist gestört. Wir arbeiten daran.“
  Was keine Regel hat, wartet auf eine Freigabe per Klick.
- **Gezielt.** An alle, an Computer nach Namensmuster (`PC-BUCHHALTUNG-*`) oder – mit Active Directory – an
  Gruppen, OUs und Benutzer. Benachrichtigungsgruppen fassen das unter einem Namen zusammen.
- **Nachvollziehbar.** Jede Meldung hält fest, wer sie veröffentlicht, freigegeben und entwarnt hat.
- **Unaufdringlich.** Eine echte Desktop-Leiste: maximierte Fenster rücken zur Seite statt verdeckt zu werden.
  Mehrere Monitore, hohe DPI und Terminalserver werden unterstützt.
- **Deine Marke.** Name, Farbe, Logo, Sprache und alle Texte auf der Leiste legst du fest.
- **Stiller Alarm** (optional). Eine Tastenkombination holt unauffällig Hilfe – am auslösenden Computer erscheint
  nichts. Siehe [Stiller Alarm](stiller-alarm.md).
- **Töne** (optional). Pro Art der Meldung ein Windows-Ton oder eine eigene WAV-Datei.
- **Für IT-Dienstleister.** Kundenserver verbinden sich ausgehend mit dem Hub. Siehe [Für IT-Dienstleister](dienstleister-hub.md).

## Wie eine Meldung läuft

1. Das Monitoring meldet ein Problem, oder ein Admin schreibt eine Meldung.
2. Passt eine [Regel](monitoring.md#regeln), wird die Meldung mit deren Worten veröffentlicht – nach einer
   optionalen Wartezeit gegen kurze Aussetzer. Ohne Regel wartet sie auf Freigabe (oder wird je nach Einstellung
   direkt veröffentlicht oder ignoriert).
3. Die Clients der ausgewählten Empfänger zeigen die Leiste.
4. Die Erholung aus dem Monitoring, ein Klick auf *Entwarnen* oder das geplante Ende entwarnt die Meldung: Die
   Leiste wird grün und verschwindet nach kurzer Zeit.

## Wo es weitergeht

| Du willst … | Lies |
|---|---|
| Notiflag in 15 Minuten ausprobieren | [Schnellstart](schnellstart.md) |
| den Server richtig betreiben | [Server installieren und betreiben](server-installation.md) |
| die Clients verteilen | [Windows-Client](windows-client.md) |
| Meldungen schreiben und verstehen | [Meldungen](meldungen.md) |
| dein Monitoring anbinden | [Monitoring anbinden](monitoring.md) |
| Datenschutz und Sicherheit prüfen | [Sicherheit und Datenschutz](sicherheit-und-datenschutz.md) |

> **Zum Namen:** Notiflag hieß früher *Statusbar*. Technische Namen tragen noch den alten Namen, zum Beispiel
> `statusbar-client.exe`, `Statusbar.msi`, `%ProgramData%\Statusbar` und das Repository
> [GerryCrooked/statusbar](https://github.com/GerryCrooked/statusbar). Die Doku nennt sie so, wie sie heißen.
