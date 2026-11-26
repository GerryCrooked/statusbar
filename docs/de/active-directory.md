---
id: active-directory
title: Active Directory
description: Anmeldung mit Verzeichniskonten und Empfänger nach AD-Gruppe, OU und Benutzer einrichten.
lang: de
slug: active-directory
order: 8
---

# Active Directory

Mit Active Directory (oder einem anderen LDAP-Verzeichnis mit AD-Attributen) melden sich Admins mit ihrem
Verzeichniskonto an, und Meldungen lassen sich an Gruppen, OUs und Benutzer schicken. Ohne Verzeichnis meldest du
dich mit dem `ADMIN_TOKEN` an und adressierst alle oder Computer nach Namen.

## Was Notiflag liest

Alle `AD_SYNC_SECONDS` (Standard 300) liest der Server mit einem Lese-Konto die Benutzer unter `AD_BASE_DN`, die
`AD_USER_FILTER` erfüllen (Standard: aktive Benutzerkonten), jeweils `sAMAccountName` und `memberOf`. Daraus
entstehen die Listen für die Empfängerauswahl. Notiflag schreibt nie ins Verzeichnis.

Meldet ein Client den angemeldeten Benutzer `anna`, prüft der Server Gruppen und OU von `anna` und zeigt die
passenden Meldungen.

## Einrichten

1. **Lese-Konto anlegen**, z. B. `svc_notiflag`, normales Benutzerkonto ohne besondere Rechte, Kennwort läuft
   nicht ab.
2. **Admin-Gruppe anlegen**, z. B. `Notiflag-Admins`, und die Admins aufnehmen.
3. In `.env`:

   ```dotenv
   AD_ENABLED=yes
   AD_URL=ldaps://dc01.example.local:636
   AD_BIND_DN=CN=svc_notiflag,OU=Service Accounts,DC=example,DC=local
   AD_BIND_PASSWORD=…
   AD_BASE_DN=DC=example,DC=local
   AD_ADMIN_GROUP=Notiflag-Admins
   ```

4. `docker compose up -d`, dann mit einem Konto aus der Admin-Gruppe anmelden (Benutzername ohne Domäne oder als
   `DOMÄNE\name`).

`AD_ADMIN_GROUP` ist der Name (CN) der Gruppe; nur direkte Mitglieder dürfen sich anmelden. Die Mitgliedschaft wird
bei jeder Anfrage erneut geprüft – wer aus der Gruppe fliegt, verliert den Zugang mit dem nächsten Abgleich.

## Verschlüsselung und Zertifikate

| `AD_URL` | Verbindung |
|---|---|
| `ldap://dc01.example.local:389` | wird per StartTLS verschlüsselt |
| `ldaps://dc01.example.local:636` | TLS von Anfang an |

Zertifikate werden immer geprüft. Stammt das Zertifikat des Domain Controllers aus einer internen CA, muss der
Container ihr vertrauen. Lege das CA-Zertifikat (PEM) in `server/ca/*.crt` und baue das Image neu, oder binde es in
das fertige Image ein:

```yaml
services:
  statusbar:
    environment:
      SSL_CERT_DIR: /etc/ssl/certs:/ca
    volumes:
      - ./ca:/ca:ro
```

```bash
cd ca && ln -sfn interne-root-ca.crt "$(openssl x509 -noout -hash -in interne-root-ca.crt).0"
```

Ohne das meldet der Abgleich `certificate verify failed`.

## Kontrolle

Die Empfängerauswahl zeigt nach dem ersten Abgleich Gruppen, OUs und Benutzer. Fehler stehen im Log:

```bash
docker compose logs statusbar | grep -i -E "ldap|directory"
```

## Grenzen

- Verschachtelte Gruppen werden nicht aufgelöst – nur direkte Mitglieder zählen.
- Ein Benutzer muss vom Filter erfasst sein, sonst gelten für ihn nur Meldungen an alle und an Computer.
- Der Client meldet den Benutzernamen selbst; das ist eine Anzeige-Steuerung, keine Zugriffskontrolle (siehe
  [Sicherheit und Datenschutz](sicherheit-und-datenschutz.md)).
