// Statusbar - Zabbix media type script (Alerts -> Media types -> Create media type, Type: Webhook).
// Easiest: import media_statusbar.yaml from this folder; it contains this script and all parameters.
//
// Parameters of the media type:
//   url        = {ALERT.SENDTO}            webhook URL, entered in the user's media ("Send to")
//   event_id   = {EVENT.ID}                stays the same for problem and recovery
//   value      = {EVENT.VALUE}             1 = problem, 0 = recovery
//   update     = {EVENT.UPDATE.STATUS}     1 = update operation (acknowledge, comment) -> skipped
//   host       = {HOST.NAME}
//   trigger    = {EVENT.NAME}
//   severity   = {EVENT.SEVERITY}          Not classified, Information, Warning, Average, High, Disaster
//   message    = {ALERT.MESSAGE}
//   proxy      = (optional) http://proxy.example.local:3128
//
// Written for Zabbix 6.0 and later (Duktape, ECMAScript 5: no arrow functions, no let/const).
var p = JSON.parse(value);

function present(s) {
    // Unresolved macros (e.g. when testing the media type) arrive literally as "{EVENT.ID}".
    return typeof s === 'string' && s !== '' && !/^\{[A-Z0-9_.]+\}$/.test(s);
}

if (!present(p.url) || p.url.indexOf('http') !== 0) {
    throw 'Statusbar: parameter "url" (Send to) must be the webhook URL https://<server>/api/hooks/<token>';
}
if (p.update === '1') {
    return 'skipped (update operation)';
}
if (p.value !== '1' && p.value !== '0') {
    throw 'Statusbar: parameter "value" must be {EVENT.VALUE} (1 = problem, 0 = recovery), got "' + p.value + '"';
}

var body = {
    key: 'zabbix-' + (present(p.event_id) ? p.event_id : p.host + '/' + p.trigger),
    status: p.value === '1' ? 'problem' : 'ok',
    host: present(p.host) ? p.host : '',
    service: present(p.trigger) ? p.trigger : '',
    severity: present(p.severity) ? p.severity : '',
    message: present(p.message) ? p.message : ''
};

var request = new HttpRequest();
if (present(p.proxy)) {
    request.setProxy(p.proxy);
}
request.addHeader('Content-Type: application/json');
Zabbix.log(4, '[Statusbar] sending ' + body.status + ' for ' + body.key);
var response = request.post(p.url, JSON.stringify(body));
var status = request.getStatus();
if (status !== 200) {
    throw 'Statusbar answered HTTP ' + status + ': ' + response;
}
// {"ok":true,"events":["created"]} - shown in the action log and in "Test" results.
return response;
