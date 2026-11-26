---
id: service-provider
title: Für IT-Dienstleister
description: Kundenserver mit dem Notiflag-Hub verbinden, Meldungen an einen, mehrere oder alle Kunden schicken, und was der Kunde dabei steuert.
lang: de
slug: dienstleister-hub
order: 12
---

# Für IT-Dienstleister

IT-Dienstleister betreiben einen zentralen **Hub** und verbinden die Notiflag-Server ihrer Kunden damit. Der Hub
ist ein eigenes, kommerzielles Produkt; die Kundenseite ist Teil jedes Notiflag-Servers.

```text
   Dienstleister                             Kunde
 ┌──────────────────────┐            ┌─────────────────────────────────────┐
 │ Hub                  │  ◄───────  │ Notiflag-Server                     │
 │  · Kundenübersicht   │   HTTPS,   │  · eigenes Verzeichnis, Branding,   │
 │  · Meldungen an      │  nur aus-  │    Monitoring, Benachrichtigungs-   │
 │    einen, mehrere    │   gehend   │    gruppen                          │
 │    oder alle Kunden  │            │  · HUB_URL + HUB_TOKEN              │
 └──────────────────────┘            │  · entscheidet: zeigen/freigeben/aus│
                                     └──────────────────┬──────────────────┘
                                                        │
                                              Arbeitsplätze des Kunden
```

Jeder Kunde behält einen vollständigen eigenen Server; er arbeitet weiter, wenn der Hub nicht erreichbar ist.

## Was der Hub kann

- **Kunden**: Kunde anlegen, zwei Zeilen (`HUB_URL`, `HUB_TOKEN`) an den Kunden übergeben. Der Kundenserver
  verbindet sich ausgehend – beim Kunden muss nichts geöffnet werden, kein VPN, keine Portfreigabe.
- **Übersicht** pro Kunde: online oder offline seit wann, Serverversion, Computer mit Client-Version und letztem
  Kontakt, aktive Meldungen, Benachrichtigungsgruppen; dazu die Summe der Computer aller Kunden (letzte 30 Tage).
- **Meldungen an Kunden**: Empfänger *Kunde* mit einem Namen, einem Muster (`Praxis*`), `*` für alle oder
  `Name/Gruppe` für eine Benachrichtigungsgruppe eines Kunden (`*/Buchhaltung` = diese Gruppe bei jedem Kunden). Pro
  Meldung sieht der Dienstleister, ob sie beim Kunden angezeigt wird, auf Freigabe wartet, entwarnt oder verworfen
  wurde – und von wem.
- **Zentrales Monitoring**: Integrationen und Regeln funktionieren im Hub wie in jedem Server; mit Empfänger *Kunde*
  speist ein zentrales Monitoring die Leisten der Kunden.
- **Stille Alarme** der Kunden sieht der Dienstleister mit Titel und Uhrzeit und kann sie beenden.

Der Hub ist selbst ein vollständiger Notiflag-Server (mit eigenen Arbeitsplätzen, falls gewünscht) und läuft wie
dieser mit Docker hinter einem HTTPS-Reverse-Proxy. Das Image und die Hub-Lizenz gibt es über
[notiflag.de](https://notiflag.de).

## Einen Kunden verbinden

**Beim Dienstleister** im Hub unter **Kunden → Kunde hinzufügen**: Name des Kunden, **Anlegen**. Der Hub zeigt
einmalig die zwei Zeilen für den Kunden.

**Beim Kunden** in der `.env` des Notiflag-Servers:

```dotenv
HUB_URL=https://hub.example-dienstleister.de
HUB_TOKEN=sbh_…
```

```bash
docker compose up -d
```

*Einstellungen* beim Kunden zeigt, ob die Verbindung steht und wann der Hub zuletzt erreicht wurde; der Hub zeigt den
Kunden als *Online*. Der Kundenserver muss `https://<hub>/api/hub/sync` erreichen.

Einen Kunden entfernen: im Hub *Löschen*. Sein Server kann sich danach nicht mehr verbinden.

## Was der Kunde steuert

*Einstellungen → Meldungen deines Dienstleisters*:

| Einstellung | Wirkung |
|---|---|
| *Sofort anzeigen* (Standard) | Meldungen vom Hub erscheinen sofort |
| *Zur Freigabe vorhalten* | Sie warten unter *Wartet auf Freigabe*, bis ein Admin des Kunden sie veröffentlicht |
| *Nicht annehmen* | Sie werden ignoriert |

Ein Admin des Kunden kann eine Meldung vom Hub jederzeit entwarnen; sie bleibt dann beim Kunden entwarnt. Eine
wartende Meldung kann er verwerfen; sie kommt nicht wieder. Jede Meldung vom Hub ist als *vom Dienstleister*
gekennzeichnet.

Der Dienstleister kann alle beim Kunden oder eine seiner **Benachrichtigungsgruppen** ansprechen. Wer in einer
Gruppe ist, entscheidet der Kunde; der Hub kennt nur den Namen.

## Was an den Hub geht

- Version und Name des Servers
- Computer, die in den letzten 30 Tagen den Client genutzt haben: Computername, Client-Version, letzter Kontakt
- Titel und Dringlichkeit aktiver Meldungen
- Namen der Benachrichtigungsgruppen
- die Einstellung für Meldungen des Dienstleisters und pro Meldung des Dienstleisters ihr Zustand beim Kunden mit
  dem Namen des lokalen Admins, der freigegeben, entwarnt oder verworfen hat
- bei einem stillen Alarm zusätzlich Kennung und Uhrzeit; sein Titel enthält meist Benutzer- und Computernamen

Nicht gesendet: Benutzernamen (außer in Alarmtiteln wie eingestellt), Meldungstexte, Verzeichnisdaten,
Einstellungen, Tokens.

## Lizenz

Kundenserver, die mit einem Hub verbunden sind, brauchen keine eigene Lizenz – unabhängig von der Zahl der PCs. Der
Hub braucht eine Hub-Lizenz (`LICENSE_KEY` im Hub). Partnerkonditionen gibt es auf Anfrage über
[notiflag.de](https://notiflag.de). Siehe [Lizenz](lizenz.md).

## Protokoll

Für Entwickler eines eigenen Hubs ist das Protokoll offen dokumentiert:
[docs/HUB.md im Repository](https://github.com/GerryCrooked/statusbar/blob/main/docs/HUB.md#protocol) (Englisch).
