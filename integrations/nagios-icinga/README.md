# Notiflag for Nagios, Naemon and Icinga

A service going CRITICAL (or a host going DOWN) opens a message on your users' desktops; the RECOVERY
notification gives it the green all-clear and takes it away.

| File | Purpose |
|---|---|
| [`../curl/notiflag-notify.sh`](../curl/notiflag-notify.sh) | The sender (POSIX shell + `curl`), shared with the [curl guide](../curl/README.md) |
| [`nagios.cfg`](nagios.cfg) | Commands and contact for **Nagios Core 3/4, Naemon, Icinga 1** |
| [`icinga2.conf`](icinga2.conf) | User, notification commands and apply rules for **Icinga 2** |

Tested end to end with Nagios Core 4.5.12 and Icinga 2.14.5 (host and service problem, acknowledgement
skipped, recovery), against a throwaway Notiflag server.

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `Nagios` (or `Icinga`), policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Install the sender (both)

On the monitoring server (Icinga 2: on the node that sends notifications, usually the master):

```bash
# curl is required; minimal images (e.g. some Nagios containers) do not ship it
apt-get install curl            # or: dnf install curl
install -m 755 notiflag-notify.sh /usr/local/bin/notiflag-notify.sh
```

Keep the webhook URL out of the configuration – it would show up in `ps`, config backups and Git:

```bash
# Nagios / Naemon / Icinga 1
install -d -m 750 -o nagios -g nagios /etc/notiflag
echo 'https://status.example.com/api/hooks/<token>' > /etc/notiflag/webhook-url
chown nagios:nagios /etc/notiflag/webhook-url; chmod 600 /etc/notiflag/webhook-url

# Icinga 2 (user "nagios" on Debian/Ubuntu, "icinga" on RHEL/SUSE)
echo 'https://status.example.com/api/hooks/<token>' > /etc/icinga2/notiflag-webhook-url
chown nagios:nagios /etc/icinga2/notiflag-webhook-url; chmod 600 /etc/icinga2/notiflag-webhook-url
```

Check it as the monitoring user:

```bash
sudo -u nagios env NOTIFLAG_URL_FILE=/etc/notiflag/webhook-url \
  /usr/local/bin/notiflag-notify.sh CRITICAL nagios-test Notiflag "This is a test"
# {"events":["draft"],"ok":true}
sudo -u nagios env NOTIFLAG_URL_FILE=/etc/notiflag/webhook-url \
  /usr/local/bin/notiflag-notify.sh OK nagios-test Notiflag
# {"events":["cancelled"],"ok":true}
```

## 3a. Nagios Core, Naemon, Icinga 1

1. Copy `nagios.cfg` to e.g. `/etc/nagios/objects/notiflag.cfg` (Naemon: `/etc/naemon/conf.d/notiflag.cfg`)
   and include it in `nagios.cfg` with `cfg_file=…` (not needed for a `cfg_dir`).
2. It defines the commands `notiflag-host` / `notiflag-service` and a **contact `notiflag`** that stands
   for "the users' screens". Add that contact to what users should hear about:

   ```
   define service {
       use                   generic-service
       host_name             mail01
       service_description   SMTP
       check_command         check_smtp
       contacts              +notiflag     ; "+" adds to the contacts from the template
   }
   ```

   or put it into a contact group used by those hosts and services.
3. Check and reload:

   ```bash
   nagios -v /etc/nagios/nagios.cfg && systemctl reload nagios
   ```

The command line passes `$NOTIFICATIONTYPE$` as `NOTIFLAG_TYPE`: acknowledgements, downtimes, flapping
and custom notifications are skipped, only PROBLEM and RECOVERY are sent. Nagios removes the characters
of `illegal_macro_output_chars` (`` ` ~ $ & | ' " < > ``) from plugin output before it reaches the script.

## 3b. Icinga 2

1. Copy `icinga2.conf` to `/etc/icinga2/conf.d/notiflag.conf`. With a cluster or Icinga Director-managed
   setup, put the objects into the master zone (`zones.d/master/`) or recreate them in the Director
   (the file maps 1:1 to a user, two notification commands and two notification apply rules).
2. Mark hosts and services users should hear about:

   ```
   object Host "mail01" {
     import "generic-host"
     address = "192.0.2.10"
     vars.notiflag = true
   }

   apply Service "smtp" {
     import "generic-service"
     check_command = "smtp"
     vars.notiflag = true
     assign where host.name == "mail01"
   }
   ```

   Or change the `assign where` lines in `notiflag.conf`, e.g. `assign where "mail-servers" in host.groups`.
3. Check and reload:

   ```bash
   icinga2 daemon -C && systemctl reload icinga2
   ```

`types = [ Problem, Recovery ]` makes Icinga call the command only for problems and recoveries;
`interval = 0` sends one notification per state change instead of re-notifying.

## 4. How Nagios / Icinga map to Notiflag

| Nagios / Icinga | Notiflag |
|---|---|
| PROBLEM notification, service `CRITICAL`, `UNKNOWN`; host `DOWN`, `UNREACHABLE` | problem, Outage (`error`) |
| PROBLEM notification, service `WARNING` | problem, Degraded (`warning`) |
| RECOVERY notification (`OK`, `UP`) | recovery: message resolved |
| ACKNOWLEDGEMENT, DOWNTIME*, FLAPPING*, CUSTOM | skipped by the script |
| `$HOSTNAME$` / `$SERVICEDESC$` (`$host.name$` / `$service.name$`) | host / service, key `host/service` |
| Host notification | key `host/`, title = host name |
| `$SERVICEOUTPUT$` / `$HOSTOUTPUT$` | message text without rule |

A host outage and the service outages on it are separate messages. If users should only see one, notify
the `notiflag` contact on the host check **or** on the services, not both – or let dependencies suppress
service notifications while the host is down (Nagios does this by default for hosts that are DOWN).

## 5. Test end to end

Nagios: *Service → Submit passive check result* (or the external command):

```bash
now=$(date +%s)
printf '[%s] PROCESS_SERVICE_CHECK_RESULT;mail01;SMTP;2;Notiflag test\n' $now > /var/lib/nagios/rw/nagios.cmd
# … the message appears; later:
printf '[%s] PROCESS_SERVICE_CHECK_RESULT;mail01;SMTP;0;OK\n' $now > /var/lib/nagios/rw/nagios.cmd
```

This needs passive checks to be accepted for that service and `max_check_attempts 1` (or several results)
to reach a HARD state. Icinga 2: *Icinga Web → service → Process check result*, or
`icinga2 api` / `curl` against `/v1/actions/process-check-result`.

Watch `/var/log/nagios/nagios.log` (Icinga 2: `/var/log/icinga2/icinga2.log`):

```
SERVICE NOTIFICATION: notiflag;mail01;SMTP;CRITICAL;notiflag-service;Notiflag test
```

## 6. Example rule

In Notiflag (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `vpn-gw?/*` |
| Show this title | `VPN connections are failing` |
| And this message | `Working from home is currently not possible. We are working on it.` |
| Severity | Outage |

`vpn-gw?/` (host problem, empty service) and `vpn-gw?/IPsec` both match.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Log: `SERVICE NOTIFICATION: notiflag…` missing | The contact `notiflag` is not on that service, notifications are disabled for it, the state is still SOFT, or `notification_options` exclude it. |
| Log: `Warning: Contact 'notiflag' service notification command '…' timed out` | Notiflag not reachable from the monitoring server; test the sender by hand (step 2). |
| Sender prints `curl: (22) … 401` | Wrong or incomplete URL in the URL file. |
| Sender prints `curl: (6) Could not resolve host` / `(7) Failed to connect` | DNS/firewall; set `https_proxy` in the command's environment if you need a proxy. |
| Sender prints `curl: (60) SSL certificate problem` | Install your internal root CA on the monitoring server (`update-ca-certificates`). |
| `notiflag: set NOTIFLAG_URL or NOTIFLAG_URL_FILE` / `Permission denied` | The URL file is missing or not readable by the monitoring user. |
| `notiflag: skipped notification type ACKNOWLEDGEMENT` | Expected. |
| Icinga 2: `Notification command … failed: … No such file or directory` | `/usr/local/bin/notiflag-notify.sh` missing or `curl` not installed. |
| Message stays | RECOVERY notifications are filtered (`notification_options` without `r`, Icinga 2 `types` without `Recovery`). |
