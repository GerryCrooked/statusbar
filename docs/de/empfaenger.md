---
id: recipients
title: Empfänger
description: Meldungen an alle, an Computer nach Namensmuster, an AD-Gruppen, OUs, Benutzer oder Benachrichtigungsgruppen schicken.
lang: de
slug: empfaenger
order: 7
---

# Empfänger

Ohne Empfänger geht eine Meldung an **alle**. Sonst sieht sie jeder Computer und jeder Benutzer, auf den
mindestens einer der gewählten Empfänger passt.

## Arten von Empfängern

| Art | Beispiel | Passt auf | Braucht |
|---|---|---|---|
| **Computer** | `PC-EMPFANG-01` | genau diesen Computer; Auswahl aus den Computern, die sich in den letzten 30 Tagen gemeldet haben | – |
| **Computer-Muster** | `PC-BUCH-*`, `TS0?` | Computernamen; `*` = beliebig viele Zeichen, `?` = genau eins, Groß-/Kleinschreibung egal | – |
| **Benachrichtigungsgruppe** | *Buchhaltung* | alles, was die Gruppe enthält | – |
| **Gruppe** | `CN=Buchhaltung,OU=Gruppen,DC=example,DC=local` | direkte Mitglieder der AD-Gruppe | Active Directory |
| **OU** | `OU=Standort Ulm,DC=example,DC=local` | Benutzer in dieser OU und darunter | Active Directory |
| **Benutzer** | `anna` | genau dieser Anmeldename (`sAMAccountName`) | Active Directory |

Im Formular wählst du die Art und tippst den Anfang des Namens; Notiflag schlägt passende Computer, Gruppen, OUs,
Benutzer und Benachrichtigungsgruppen vor. Höchstens 50 Empfänger pro Meldung.

**Gut zu wissen:**

- Computer-Muster vergleichen den **Computernamen**, den der Client meldet – auf einem Terminalserver ist das der
  Name des Terminalservers.
- AD-Gruppen zählen nur **direkte** Mitglieder; verschachtelte Gruppen werden nicht aufgelöst.
- Benutzer, Gruppen und OUs funktionieren nur, wenn der angemeldete Windows-Benutzer im Verzeichnis gefunden wird
  (siehe [Active Directory](active-directory.md)).

## Benachrichtigungsgruppen

Eine Benachrichtigungsgruppe fasst beliebige Empfänger unter einem Namen zusammen – zum Beispiel *Buchhaltung* =
OU Buchhaltung + Computer `PC-BUCH-*` + Benutzer `chefin`.

**Einstellungen → Benachrichtigungsgruppen → Gruppe anlegen oder ändern**: Name, Mitglieder (dieselben Arten wie
oben), **Speichern**. Danach steht die Gruppe bei jeder Meldung, Regel und Integration als Empfänger zur Wahl.

Warum Gruppen?

- Wer dazugehört, pflegst du an einer Stelle; Regeln und Integrationen bleiben unverändert.
- Mit einem [Hub](dienstleister-hub.md) sieht dein Dienstleister nur die **Namen** der Gruppen und kann sie
  ansprechen – wer drin ist, entscheidest du.
- Der [stille Alarm](stiller-alarm.md) geht typischerweise an eine Gruppe wie *Empfang und Haustechnik*.

## Wo Empfänger gesetzt werden

| Ort | Wirkung |
|---|---|
| Neue Meldung | Empfänger dieser Meldung |
| Regel (Monitoring) | Empfänger aller Meldungen, auf die die Regel passt |
| Integration → *Standard-Empfänger* | Empfänger von Meldungen dieser Integration ohne passende Regel |
| Stiller Alarm | Wer den Alarm sieht |
| Hub des Dienstleisters | Alle bei dir oder eine deiner Benachrichtigungsgruppen |

Empfänger entscheiden, **wer eine Meldung angezeigt bekommt** – nicht, wer sie lesen könnte. Vertrauliches gehört
nicht in Meldungen (siehe [Sicherheit und Datenschutz](sicherheit-und-datenschutz.md)).
