// Zabbix media type "Webhook" script (Alerts -> Media types -> Create, Type: Webhook).
//
// Parameters to define on the media type:
//   url       = https://status.example.com/api/hooks/<integration token>
//   event_id  = {EVENT.ID}
//   value     = {EVENT.VALUE}
//   update    = {EVENT.UPDATE.STATUS}
//   host      = {HOST.NAME}
//   trigger   = {EVENT.NAME}
//   severity  = {EVENT.SEVERITY}
//   message   = {ALERT.MESSAGE}
//
// Then add the media to a user and enable it in a trigger action
// (operations AND recovery operations, so problems are resolved automatically).
var p = JSON.parse(value);
if (p.update === '1') {
    return 'skipped (update operation)';
}
var request = new HttpRequest();
request.addHeader('Content-Type: application/json');
var response = request.post(p.url, JSON.stringify({
    key: 'zabbix-' + p.event_id,
    status: p.value === '1' ? 'problem' : 'ok',
    host: p.host,
    service: p.trigger,
    severity: p.severity,
    message: p.message
}));
if (request.getStatus() !== 200) {
    throw 'Statusbar answered HTTP ' + request.getStatus() + ': ' + response;
}
return response;
