# Monitoring integrations

One folder per monitoring system, each with a step-by-step guide, the script or configuration to
install, and sample payloads for testing. How Notiflag reads alerts – problem and recovery, keys,
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
| [librenms](librenms/README.md) | LibreNMS (API transport) |
| [microsoft-365](microsoft-365/README.md) | Microsoft 365 service health (the server polls Microsoft Graph) – *in development* |
| [power-automate](power-automate/README.md) | Power Automate / Logic Apps, e.g. alert e-mails from devices that can only send mail |
| [zammad](zammad/README.md) | Zammad (webhooks + triggers) |
| [glpi](glpi/README.md) | GLPI (webhooks) |
| [znuny-otrs](znuny-otrs/README.md) | Znuny and OTRS (Generic Agent module) |
| [jira-service-management](jira-service-management/README.md) | Jira Service Management (automation) |
| [servicenow](servicenow/README.md) | ServiceNow (business rule) |
| [topdesk](topdesk/README.md) | TOPdesk (Action Management) |
| [azure-monitor](azure-monitor/README.md) | Azure Monitor: metric, log, Service Health and Resource Health alerts |
| [ninjaone](ninjaone/README.md) | NinjaOne (notification channel or API webhook) – *in development* |
| [datto-rmm](datto-rmm/README.md) | Datto RMM (monitor webhooks) |
| [n-able](n-able/README.md) | N-able N-central (HTTP-XML notifications) |
| [atera](atera/README.md) | Atera (poller script at the service provider) – *in development* |
| [tactical-rmm](tactical-rmm/README.md) | Tactical RMM (webhooks in alert templates) |
| [other-rmm](other-rmm/README.md) | ConnectWise Automate/RMM, Kaseya VSA, N-sight and others |
| [curl](curl/README.md) | curl, shell scripts, cron and backup jobs, PowerShell |

`scripts/test-integrations.sh` sends every sample payload and runs every bundled sender against a
Notiflag server and checks that problems open and recoveries resolve messages.
