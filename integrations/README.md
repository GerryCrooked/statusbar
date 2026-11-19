# Monitoring integrations

One folder per monitoring system, each with a step-by-step guide, the script or configuration to
install, and sample payloads for testing. How Statusbar reads alerts – problem and recovery, keys,
rules, outcomes – is explained in [docs/INTEGRATIONS.md](../docs/INTEGRATIONS.md).

| Folder | System |
|---|---|
| [zabbix](zabbix/README.md) | Zabbix 6.0+ (media type) |
| [checkmk](checkmk/README.md) | Checkmk 2.0+ (notification script) |
| [prtg](prtg/README.md) | PRTG (PowerShell script or HTTP action) |
| [nagios-icinga](nagios-icinga/README.md) | Nagios Core, Naemon, Icinga 1 and Icinga 2 |
| [alertmanager](alertmanager/README.md) | Prometheus Alertmanager |
| [grafana](grafana/README.md) | Grafana alerting |
| [uptime-kuma](uptime-kuma/README.md) | Uptime Kuma |
| [curl](curl/README.md) | curl, shell scripts, cron and backup jobs, PowerShell |

`scripts/test-integrations.sh` sends every sample payload and runs every bundled sender against a
Statusbar server and checks that problems open and recoveries resolve messages.
