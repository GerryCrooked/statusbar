---
id: active-directory
title: Active Directory
description: Set up sign-in with directory accounts and recipients by AD group, OU and user.
lang: en
slug: active-directory
order: 8
---

# Active Directory

With Active Directory (or another LDAP directory with AD attributes) admins sign in with their directory account,
and messages can be sent to groups, OUs and users. Without a directory you sign in with the `ADMIN_TOKEN` and address
everyone or computers by name.

## What Notiflag reads

Every `AD_SYNC_SECONDS` (default 300) the server reads, with a read-only account, the users below `AD_BASE_DN` that
match `AD_USER_FILTER` (default: enabled user accounts), each with `sAMAccountName` and `memberOf`. The recipient
picker lists are built from that. Notiflag never writes to the directory.

When a client reports the signed-in user `anna`, the server checks anna's groups and OU and shows the matching
messages.

## Set up

1. **Create a read-only account**, e.g. `svc_notiflag`: a normal user account without special rights, password does
   not expire.
2. **Create an admin group**, e.g. `Notiflag-Admins`, and add the admins.
3. In `.env`:

   ```dotenv
   AD_ENABLED=yes
   AD_URL=ldaps://dc01.example.local:636
   AD_BIND_DN=CN=svc_notiflag,OU=Service Accounts,DC=example,DC=local
   AD_BIND_PASSWORD=…
   AD_BASE_DN=DC=example,DC=local
   AD_ADMIN_GROUP=Notiflag-Admins
   ```

4. `docker compose up -d`, then sign in with an account from the admin group (user name without domain, or as
   `DOMAIN\name`).

`AD_ADMIN_GROUP` is the group's name (CN); only direct members may sign in. Membership is checked again on every
request – whoever leaves the group loses access with the next sync.

## Encryption and certificates

| `AD_URL` | Connection |
|---|---|
| `ldap://dc01.example.local:389` | encrypted with StartTLS |
| `ldaps://dc01.example.local:636` | TLS from the start |

Certificates are always verified. If the domain controller's certificate comes from an internal CA, the container
must trust it. Put the CA certificate (PEM) in `server/ca/*.crt` and rebuild the image, or mount it into the released
image:

```yaml
services:
  statusbar:
    environment:
      SSL_CERT_DIR: /etc/ssl/certs:/ca
    volumes:
      - ./ca:/ca:ro
```

```bash
cd ca && ln -sfn internal-root-ca.crt "$(openssl x509 -noout -hash -in internal-root-ca.crt).0"
```

Without it the sync logs `certificate verify failed`.

## Check

After the first sync the recipient picker shows groups, OUs and users. Errors are in the log:

```bash
docker compose logs statusbar | grep -i -E "ldap|directory"
```

## Limits

- Nested groups are not expanded – only direct members count.
- A user must be covered by the filter; otherwise only messages to everyone and to computers apply to them.
- The client reports the user name itself; this controls what is displayed, it is not access control (see [Security
  and privacy](security-and-privacy.md)).
