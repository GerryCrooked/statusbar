---
id: silent-alarm
title: Stiller Alarm
description: Mit einer Tastenkombination unauffällig Hilfe holen – einrichten, testen, Grenzen und Datenschutz.
lang: de
slug: stiller-alarm
order: 9
---

# Stiller Alarm

Ein Anwender drückt an seinem Computer eine Tastenkombination, zum Beispiel `Strg+Umschalt+F12`. Sofort sehen die
gewählten Empfänger eine violette Meldung wie „Stiller Alarm von anna an PC-EMPFANG-01“. Am auslösenden Computer
erscheint **nichts** – keine Bestätigung, kein Ton, auch nicht in der eigenen Leiste.

Gedacht für Empfang, Schalter, Praxen, Kanzleien und alle, die unauffällig Hilfe holen müssen.

> **Kein Notrufsystem.** Der stille Alarm hängt am Netz, am Server und daran, dass jemand auf einen Bildschirm
> schaut. Es gibt keine Rückmeldung an die auslösende Person. Behalte eure regulären Notfallabläufe und vereinbart,
> was ein Alarm bedeutet und wer nachsieht.

## Einschalten

**Einstellungen → Stiller Alarm**:

| Einstellung | Standard | Hinweise |
|---|---|---|
| *Stillen Alarm auf allen Computern erlauben* | aus | Schaltet die Tastenkombination auf allen Clients ein; aus = sie wird innerhalb von Sekunden wieder entfernt |
| *Tastenkombination* | `Ctrl+Shift+F12` | Strg, Alt, Umschalt oder Win plus eine Funktionstaste, oder zwei davon plus ein Zeichen (`Ctrl+Shift+Ü`) |
| *Titel* | „Silent alarm from {user} at {computer}“ | `{user}` und `{computer}` werden ersetzt. Auf Deutsch z. B. „Stiller Alarm: {user} an {computer}“ |
| *Meldung* | „Please check on them now.“ | Was die Empfänger tun sollen, z. B. „Bitte sofort nachsehen.“ |
| *Empfänger* | alle | Am besten eine [Benachrichtigungsgruppe](empfaenger.md#benachrichtigungsgruppen) wie *Empfang und Haustechnik* |
| *Wiederholungen ignorieren für (s)* | 300 | Nach dem Ende eines Alarms ignoriert Notiflag weitere Auslösungen derselben Sitzung so lange |
| *Beenden nach (min, 0 = bei Erledigt)* | 0 | 0 = der Alarm bleibt, bis jemand ihn entwarnt |

Zusätzlich lassen sich unter *Töne* ein eigener Ton und unter *Bezeichnungen vor dem Titel* ein eigenes Etikett für
den stillen Alarm festlegen.

**Tastenkombination wählen:** Sie muss Strg, Alt oder Win enthalten, bei einem Zeichen zwei Zusatztasten – so
bleiben normales Tippen und Kürzel wie Strg+C frei. Wähle eine Kombination, die niemand versehentlich drückt und
kein anderes Programm belegt. Zeichen wie `Ü` gibt es nur auf passenden Tastaturlayouts.

## Was die Empfänger sehen

- Eine **violette** Meldung mit eigenem Etikett.
- Sie bleibt sichtbar: Gibt es gleichzeitig andere Meldungen, behält der Alarm das linke Feld der Leiste (40 %),
  die anderen wechseln rechts.
- Sie erscheint auch über Vollbildfenstern und über einer Remotedesktop-Sitzung im Modus *Ausblenden*.
- Anwender können sie nicht wegklicken. Sie endet, wenn ein Admin sie unter **Meldungen** entwarnt, der
  Dienstleister sie im Hub beendet oder die eingestellte Zeit abläuft.

## Testen

1. Einschalten, Empfänger = eine Testgruppe mit deinem eigenen PC (nicht dem, an dem du drückst).
2. An einem zweiten PC die Kombination drücken. Dort erscheint nichts.
3. Dein PC zeigt die violette Meldung; unter **Meldungen → Aktiv** steht sie mit Herkunft *stiller Alarm*.
4. **Entwarnen**.

Funktioniert die Kombination an einem PC nicht, steht der Grund in `%LOCALAPPDATA%\Statusbar\client.log` (zum
Beispiel: schon von einem anderen Programm belegt, oder das Zeichen fehlt im Tastaturlayout).

## Grenzen gegen Fehlalarme und Fluten

- Solange ein Alarm einer Sitzung offen ist, ändert erneutes Drücken dort nichts.
- Der Client ignoriert ein zweites Drücken innerhalb von 10 Sekunden.
- Nach dem Ende ignoriert der Server weitere Auslösungen derselben Sitzung für die eingestellte Sperrzeit
  (Standard 5 Minuten).
- Höchstens 5 Alarme pro 10 Minuten pro Server; weitere werden abgelehnt und protokolliert.
- Ist der Server nicht erreichbar, versucht der Client es etwa eine Minute lang.

## Besonderheiten

- **Remotedesktop im Vollbild:** Tasten gehen an die Remotesitzung. Ist der Client auf dem Terminalserver
  installiert, löst er dort aus – mit dem Namen des Terminalservers als Computer.
- **Terminalserver:** „Nichts anzeigen am auslösenden Computer“ gilt für alle Sitzungen auf diesem Server.
- **Ältere Clients** zeigen Alarme in der Farbe einer Störung und lassen sie mit anderen Meldungen wechseln.
- **Dienstleister mit Hub:** Der Dienstleister sieht aktive Alarme mit Titel, Kennung und Uhrzeit und kann sie
  beenden – unabhängig von der Einstellung für seine Meldungen.

## Datenschutz und Nachweise

- Jeder Alarm ist eine Meldung mit der Herkunft `alarm:<benutzer>@<computer>` und bleibt wie andere Meldungen
  `RETENTION_DAYS` Tage gespeichert.
- Jede angenommene, wiederholte, begrenzte oder abgelehnte Auslösung steht im Server-Log mit Benutzer, Computer
  und Absenderadresse (`X-Forwarded-For` des Reverse-Proxys).
- Benutzer- und Computername meldet der Client selbst; sie sind nicht kryptografisch geprüft. Der Server nimmt nur
  Computer an, die sich in den letzten 30 Tagen gemeldet haben.
- Der Titel enthält meist den Benutzernamen. Informiere die Anwender, bevor du den stillen Alarm einschaltest, und
  binde – wo vorhanden – Datenschutzbeauftragte und Betriebsrat ein.

Mehr zum Sicherheitsmodell: [Sicherheit und Datenschutz](sicherheit-und-datenschutz.md).
