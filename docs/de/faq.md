---
id: faq
title: Häufige Fragen
description: Antworten auf häufige Fragen zu Notiflag – Betriebssysteme, Cloud, Kosten, Datenschutz und Grenzen.
lang: de
slug: faq
order: 17
---

# Häufige Fragen

**Läuft der Client auch auf macOS oder Linux?**
Nein, derzeit nur auf Windows 10 und 11 sowie auf Windows-Terminalservern.

**Gibt es Notiflag als Cloud-Dienst?**
Nein. Der Server läuft bei dir (oder bei deinem IT-Dienstleister). Es gibt keine Cloud und keine Telemetrie.

**Muss der Server aus dem Internet erreichbar sein?**
Nein, wenn Clients und Monitoring im selben Netz sind. Nur Cloud-Dienste (z. B. Azure Monitor, Jira Cloud, NinjaOne)
brauchen eine öffentliche HTTPS-Adresse – dafür reicht `/api/hooks/`.

**Was kostet es?**
Bis 10 PCs nichts, mit allen Funktionen. Darüber gibt es Lizenzen pro PC; Preise auf [notiflag.de](https://notiflag.de).
Kundenserver an einem Hub brauchen keine eigene Lizenz. Siehe [Lizenz](lizenz.md).

**Was passiert, wenn die Lizenz abläuft?**
Nichts wird gesperrt. Die Weboberfläche zeigt einen Hinweis, Meldungen funktionieren weiter.

**Wie schnell erscheint eine Meldung?**
In etwa einer Sekunde. Die Clients halten eine offene Verbindung zum Server.

**Was sehen Anwender, wenn der Server ausfällt?**
Den letzten bekannten Stand. Der Client erfindet keine Entwarnung und verbindet sich neu, sobald der Server wieder
da ist.

**Können Anwender die Leiste wegklicken?**
Nein. Sie verschwindet, wenn die Meldung entwarnt ist.

**Verdeckt die Leiste meine Fenster?**
Nein – sie ist eine echte Desktop-Leiste, maximierte Fenster rücken zur Seite. Über Vollbildfenstern legt sie sich
standardmäßig über den Rand; das lässt sich abschalten. Siehe [Windows-Client](windows-client.md#vollbild-und-remotedesktop).

**Brauche ich Active Directory?**
Nein. Ohne AD meldest du dich mit dem `ADMIN_TOKEN` an und schickst Meldungen an alle oder an Computer nach Namen.
Mit AD kommen Gruppen, OUs und Benutzer dazu.

**Funktioniert es mit Entra ID (Azure AD) ohne lokales AD?**
Die Anmeldung und Empfänger nach Gruppe brauchen derzeit ein LDAP-fähiges Verzeichnis. Ohne das funktionieren
Meldungen an alle, an Computer und an Benachrichtigungsgruppen aus Computern.

**Kann ich Meldungen planen?**
Ja: *Anzeigen ab* und *Automatisch entwarnen um* bei jeder Meldung, z. B. für Wartungsfenster.

**Welche Sprachen?**
Die Weboberfläche gibt es auf Deutsch und Englisch, weitere Sprachen lassen sich als JSON-Datei ergänzen. Die Texte
auf der Leiste schreibst du selbst.

**Ist der stille Alarm ein Notrufsystem?**
Nein. Er ist ein zusätzlicher, unauffälliger Kanal ohne Empfangsbestätigung. Siehe [Stiller Alarm](stiller-alarm.md).

**Ist der Quellcode offen?**
Ja, Server und Client stehen unter der AGPL-3.0 auf [GitHub](https://github.com/GerryCrooked/statusbar). Der Hub für
Dienstleister ist kommerziell.

**Warum heißen Dateien noch „Statusbar“?**
Notiflag hieß früher Statusbar. Programm-, Installer- und Ordnernamen tragen noch den alten Namen.
