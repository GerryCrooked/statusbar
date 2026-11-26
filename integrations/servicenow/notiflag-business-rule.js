// Notiflag for ServiceNow – business rule on table "incident" (Advanced, When: after, Insert + Update).
// A priority 1 incident shows a message on every desktop; resolving, closing or cancelling it – or
// lowering its priority – sends the all-clear. The webhook URL lives in the system property
// "notiflag.webhook_url" (type string, read roles: admin), so it never appears in the script.
(function executeRule(current, previous /*null when async*/) {
    var url = gs.getProperty('notiflag.webhook_url', '');
    if (!url) {
        gs.warn('Notiflag: system property notiflag.webhook_url is not set');
        return;
    }

    // What makes an incident visible to users. Alternatives: a major incident
    // (current.getValue('major_incident_state') == 'accepted') or a field of your own.
    function flagged(rec) {
        return rec && rec.getValue('priority') == '1';
    }
    var DONE = ['6', '7', '8']; // Resolved, Closed, Canceled

    var isFlagged = flagged(current);
    var wasFlagged = current.operation() == 'update' && flagged(previous);
    if (!isFlagged && !wasFlagged) {
        return;
    }
    if (current.operation() == 'update' && !current.state.changes() && !current.priority.changes()
        && !current.short_description.changes()) {
        return; // a comment or work note: nothing changes for users
    }
    var done = DONE.indexOf(current.getValue('state')) >= 0 || !isFlagged;

    var body = {
        key: 'snow-' + current.getValue('number'),
        status: done ? 'ok' : 'problem',
        host: current.cmdb_ci.getDisplayValue() || current.assignment_group.getDisplayValue(),
        service: current.business_service.getDisplayValue() || current.category.getDisplayValue(),
        title: current.getValue('short_description') || current.getValue('number'),
        severity: current.getValue('impact') == '1' ? 'error' : 'warning'
    };

    var request = new sn_ws.RESTMessageV2();
    request.setEndpoint(url);
    request.setHttpMethod('post');
    request.setRequestHeader('Content-Type', 'application/json');
    request.setRequestBody(JSON.stringify(body)); // JSON.stringify escapes quotes and backslashes
    // Notiflag on your internal network: send through a MID Server.
    // request.setMIDServer('mid-server-name');
    request.executeAsync();
})(current, previous);
