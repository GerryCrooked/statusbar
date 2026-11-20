# Statusbar with Power Automate: alert e-mails to messages

Many devices and services can only send an **e-mail** when something goes wrong: UPS units, NAS boxes,
backup appliances, printers, the internet provider's outage notices, a SaaS vendor's status mails.
A Power Automate flow on a mailbox turns those e-mails into Statusbar messages – and the "back to normal"
e-mail into the all-clear.

The same works with any other Power Automate trigger (a Microsoft Forms answer, a SharePoint list, a
Teams message, an Azure DevOps event): the flow ends with one HTTP request to Statusbar.

**Not tested in Power Automate.** The request is the generic payload tested by
`scripts/test-integrations.sh`; the flow steps follow the Power Automate designer.

Requirements:

* The **HTTP** action is a *premium* connector: the flow owner needs a Power Automate Premium (or
  Process) license. Without one, build the same flow as an Azure **Logic App** (consumption plan, billed
  per run, no license).
* Statusbar reachable from the internet over HTTPS with a publicly trusted certificate – flows run in
  Microsoft's cloud. (With an on-premises data gateway you can also reach internal servers; that needs
  a custom connector.)

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Mail alerts`, policy *Hold for approval*, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. A mailbox for alerts

Use a shared mailbox, e.g. `alerts@example.com`, and point the devices' alert e-mails at it (or forward
them with a mail flow rule). The flow owner needs *Full Access* to the shared mailbox.

## 3. Build the flow

Example: a UPS that writes *"UPS on battery"* when the power fails and *"Power restored"* afterwards.

1. *Power Automate → Create → Automated cloud flow*, name `Statusbar – UPS`.
2. Trigger **When a new email arrives in a shared mailbox (V2)** (Office 365 Outlook):
   * Original Mailbox Address: `alerts@example.com`
   * *Show all*: **From** `ups01@example.com` (one flow per device type keeps it simple)
3. **Condition**: *Subject* **contains** `on battery`.
4. *If yes* → action **HTTP**:
   * Method `POST`, URI: the webhook URL from step 1
   * Headers: `Content-Type` = `application/json`
   * Body:
     ```json
     {
       "key": "ups01-power",
       "status": "problem",
       "host": "ups01",
       "service": "Power",
       "severity": "warning",
       "message": "@{triggerOutputs()?['body/subject']}"
     }
     ```
5. *If no* → another **Condition**: *Subject* contains `Power restored` → *If yes* → **HTTP** with the same
   settings and the body
   ```json
   {"key": "ups01-power", "status": "ok", "host": "ups01", "service": "Power"}
   ```
6. *Save*.

The `key` must be the same in both bodies – that is how the all-clear finds its message. Keep the JSON
body an object as shown: Power Automate then escapes quotes in the inserted subject correctly.

Instead of the subject you can insert other fields from the trigger: *From* (`body/from`), the plain-text
body (`body/bodyPreview`), the time received. Do not insert the whole HTML body – Statusbar keeps at most
1500 characters of text.

## 4. Test

*Test → Manually*, then send an e-mail to the shared mailbox from the device's address (or with a mail flow
rule, from yourself) with the subject `UPS on battery`. The run history shows the HTTP action's answer:
`{"events":["draft"],"ok":true}`. A second e-mail `Power restored` answers `cancelled` (the draft is deleted)
or `resolved`.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `ups*/Power` | `Power failure in the building` | `Servers are running on battery. Please save your work.` | Degraded |

## Troubleshooting

| Symptom | Fix |
|---|---|
| Flow does not start | Wrong mailbox address, no *Full Access*, or the *From* filter does not match exactly. |
| HTTP action: `401` | Webhook URL incomplete or integration deleted. |
| HTTP action: `400 JSON body required` | The body was entered as text with the inserted value outside quotes; enter it as the JSON object shown. |
| HTTP action is not offered | No premium license; use a Logic App. |
| All-clear never comes | Different `key` in the two bodies, or the "restored" e-mail's subject does not match the condition. |
