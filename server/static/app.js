'use strict';
// Admin UI. No framework, no build step. All server text is inserted as text nodes.

const $ = (id) => document.getElementById(id);
let strings = {};
let meta = {};
let settings = {};
let directory = { directory: false, groups: [], ous: [], users: [] };
let timer = null;
let onScreen = [];
const TABS = ['incidents', 'monitoring', 'deploy', 'settings'];
// Add-on hooks (see Statusbar.extend at the end of this file).
const ext = { kinds: [], meta: [], loaders: {} };
const extKind = (k) => ext.kinds.find((x) => x.kind === k);

const t = (key, fallback) => strings[key] || fallback || key;

function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k === 'class') el.className = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) el.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null) el.append(c);
  return el;
}

async function api(method, path, body) {
  const opts = { method, credentials: 'same-origin', headers: {} };
  if (body instanceof Blob) opts.body = body;
  else if (body !== undefined) { opts.headers['Content-Type'] = 'application/json'; opts.body = JSON.stringify(body); }
  const res = await fetch(path, opts);
  const text = await res.text();
  if (!res.ok) { const e = new Error(text || res.statusText); e.status = res.status; throw e; }
  try { return JSON.parse(text); } catch { return text; }
}

function say(id, text, good) {
  const el = $(id);
  el.textContent = text || '';
  el.className = 'msg ' + (good ? 'good' : 'bad');
}

const when = (ts) => new Date(ts * 1000).toLocaleString(meta.language || undefined, { dateStyle: 'short', timeStyle: 'short' });

// ---- recipients picker -------------------------------------------------------

// With add-on recipient kinds, "no recipients" means this server's own desktops only.
const everyone = () => (ext.kinds.length ? t('everyone_here', 'Everyone on this server') : t('everyone', 'Everyone'));

// Who did something: a user name, "api", "monitoring:<name>", "hub:<name>", "schedule".
function actorLabel(a) {
  if (!a) return null;
  if (a === 'api') return t('actor_api', 'API');
  if (a === 'schedule') return t('actor_schedule', 'schedule');
  if (a.startsWith('monitoring:')) return a.slice(11) + ' (' + t('actor_monitoring', 'monitoring') + ')';
  if (a.startsWith('hub:')) return a.slice(4) + ' (' + t('actor_hub', 'service provider') + ')';
  if (a.startsWith('alarm:')) return a.slice(6) + ' (' + t('actor_alarm', 'silent alarm') + ')';
  return a;
}

function targetLabel(target) {
  const i = target.indexOf(':');
  const kind = target.slice(0, i), value = target.slice(i + 1);
  if (extKind(kind) && extKind(kind).labelFor) return extKind(kind).labelFor(value);
  if (kind === 'host') {
    return /[*?]/.test(value) ? t('kind_host_pattern', 'Computer pattern') + ': ' + value : t('kind_computer', 'Computer') + ': ' + value.toUpperCase();
  }
  const short = kind === 'group' || kind === 'ou' ? value.split(',')[0].replace(/^(cn|ou)=/i, '') : value;
  return t('kind_' + kind, kind) + ': ' + short;
}

// members: true for editing a notification group (plain recipients only, no nested groups).
function picker(host, members) {
  const state = [];
  const chips = h('div', { class: 'chips' });
  const kind = h('select', {});
  const value = h('input', { list: host.id + 'List', placeholder: 'PC-ACCOUNTING-*' });
  const list = h('datalist', { id: host.id + 'List' });
  const plus = h('button', { type: 'button', class: 'btn ghost small', onclick: () => add() }, '+');
  // "Computer": tick connected computers (several at once); "Computer pattern": free text such as PC-FIN-*.
  const filter = h('input', { type: 'search', placeholder: t('filter', 'Filter') });
  const checks = h('div', { class: 'checklist' });
  const picks = h('div', { class: 'picks', hidden: '' }, filter, checks);
  const drawChecks = () => {
    const q = filter.value.trim().toLowerCase();
    const names = (directory.computers || []).filter((n) => !q || n.toLowerCase().includes(q));
    checks.replaceChildren(...(names.length ? names.slice(0, 1000).map((n) => {
      const target = 'host:' + n;
      const box = h('input', { type: 'checkbox' });
      box.checked = state.some((x) => x.toLowerCase() === target.toLowerCase());
      box.addEventListener('change', () => {
        const at = state.findIndex((x) => x.toLowerCase() === target.toLowerCase());
        if (box.checked && at < 0) state.push(target);
        if (!box.checked && at >= 0) state.splice(at, 1);
        draw();
      });
      return h('label', { class: 'check' }, box, h('span', {}, n));
    }) : [h('div', { class: 'empty' }, (directory.computers || []).length ? t('no_match', 'No match.') : t('no_computers', 'No computer has connected yet.'))]));
  };
  filter.addEventListener('input', drawChecks);
  const draw = () => {
    chips.replaceChildren(...(state.length
      ? state.map((target, i) => h('span', { class: 'chip', title: target }, targetLabel(target),
          h('button', { type: 'button', 'aria-label': 'remove', onclick: () => { state.splice(i, 1); draw(); } }, '×')))
      : [h('span', { class: 'chip' }, members ? t('no_members', 'No members yet') : everyone())]));
    if (!picks.hidden) drawChecks();
  };
  const options = () => {
    const kinds = [['computer', t('kind_computer', 'Computer')], ['host', t('kind_host_pattern', 'Computer pattern')]];
    if (directory.directory) kinds.push(['group', t('kind_group', 'Group')], ['ou', t('kind_ou', 'OU')], ['user', t('kind_user', 'User')]);
    if (!members && (directory.teams || []).length) kinds.push(['team', t('kind_team', 'Notification group')]);
    if (!members) for (const k of ext.kinds) kinds.push([k.kind, k.label]);
    kind.replaceChildren(...kinds.map(([v, label]) => h('option', { value: v }, label)));
    fill();
  };
  const fill = () => {
    const pick = kind.value === 'computer';
    picks.hidden = !pick; value.hidden = pick; plus.hidden = pick;
    if (pick) { filter.value = ''; drawChecks(); }
    const added = extKind(kind.value);
    const source = added ? added.options() : { group: directory.groups, ou: directory.ous, user: directory.users,
      team: directory.teams }[kind.value] || [];
    list.replaceChildren(...source.slice(0, 2000).map((v) => h('option', { value: v })));
    value.placeholder = added ? added.placeholder || '' : kind.value === 'host' ? 'PC-ACCOUNTING-*' : '';
  };
  const add = () => {
    const v = value.value.trim();
    if (!v || kind.value === 'computer') return;
    const target = kind.value + ':' + v;
    if (!state.includes(target)) state.push(target);
    value.value = '';
    draw();
  };
  kind.addEventListener('change', fill);
  value.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); add(); } });
  host.replaceChildren(
    h('div', { class: 'row' }, kind, value, plus),
    list, picks, chips);
  options(); draw();
  // A value typed but not yet added with + counts as added, so nothing goes to everyone by accident.
  return { get: () => { add(); return state.slice(); }, clear: () => { state.length = 0; draw(); },
    set: (list) => { state.length = 0; state.push(...list); draw(); }, refresh: () => { options(); draw(); } };
}

let pickers = {};

// ---- incidents -----------------------------------------------------------------

let severity = 'error';

function drawSeverity() {
  $('iSeverity').replaceChildren(...['error', 'warning', 'info'].map((v) =>
    h('button', { type: 'button', 'data-v': v, class: v === severity ? 'on' : '', onclick: () => { severity = v; drawSeverity(); drawPreview(); } },
      t('sev_' + v, v))));
}

const barText = (label, title, message) =>
  (label ? label + ': ' : '') + title + (message && message !== title ? '  \u2013  ' + message.replace(/\s+/g, ' ') : '');

// The stage shows what desks see: the draft while you type, otherwise the live state.
function drawPreview() {
  const title = $('iTitle').value.trim();
  const bar = $('preview');
  $('stage').classList.toggle('bottom', settings.position === 'bottom');
  bar.hidden = false;
  if (title) {
    bar.className = 'bar ' + severity;
    bar.textContent = barText(settings['label_' + severity], title, $('iMessage').value.trim());
    $('stageTitle').textContent = t('stage_draft', 'This is how it will look');
    $('stageNote').textContent = t('stage_draft_note', 'Nobody sees it until you publish.');
  } else if (onScreen.length) {
    const kindOf = (i) => (i.status === 'resolved' ? 'resolved' : i.severity);
    const part = (i) => h('span', { class: 'part ' + kindOf(i) }, barText(settings['label_' + kindOf(i)], i.title, i.message));
    // An active silent alarm keeps the left part of the bar while other messages take turns on the right.
    const alarms = onScreen.filter((i) => i.status === 'active' && i.severity === 'alarm');
    const others = onScreen.filter((i) => !alarms.includes(i));
    if (alarms.length && others.length) {
      bar.className = 'bar split';
      bar.replaceChildren(part(alarms[0]), part(others[0]));
    } else {
      const i = alarms[0] || onScreen[0];
      bar.className = 'bar ' + kindOf(i);
      bar.textContent = barText(settings['label_' + kindOf(i)], i.title, i.message);
    }
    $('stageTitle').textContent = onScreen.length === 1 ? t('stage_one', 'One message is on screen')
      : onScreen.length + ' ' + t('stage_many', 'messages are on screen');
    $('stageNote').textContent = t('stage_live_note', 'Desks show this bar right now. Several messages take turns.');
  } else {
    bar.hidden = true;
    $('stageTitle').textContent = t('stage_quiet', 'All quiet');
    $('stageNote').textContent = t('stage_quiet_note', 'No bar on any desk. Publish a message or let your monitoring do it.');
  }
}

// The ticket Statusbar opened for a message: a link once it exists.
function ticketMeta(i) {
  const tk = i.ticket;
  if (!tk) return null;
  if (tk.state === 'queued') return t('tk_meta_queued', 'Ticket is being opened…');
  if (tk.state === 'failed') return h('span', { style: 'color: var(--red)' }, t('tk_meta_failed', 'Ticket could not be opened'));
  const label = t('tk_meta_ticket', 'Ticket') + ' ' + (tk.reference || tk.id || '');
  return tk.url ? h('span', {}, h('a', { href: tk.url, target: '_blank', rel: 'noopener noreferrer' }, label)) : label;
}

function incidentRow(i, now) {
  const draft = i.status === 'draft', discarded = i.status === 'discarded';
  const done = i.status === 'resolved' || discarded;
  const waiting = i.status === 'active' && i.starts_at > now;
  const by = (who) => (actorLabel(who) ? ' ' + t('by', 'by') + ' ' + actorLabel(who) : '');
  const metaLine = [when(i.created_at) + by(i.created_by),
    t('to', 'to') + ' ' + (i.targets.length ? i.targets.map(targetLabel).join(', ') : everyone()),
    i.approved_by ? t('approved', 'approved') + by(i.approved_by) : null,
    waiting ? t('starts', 'Show from') + ' ' + when(i.starts_at) : null,
    i.ends_at && !done ? t('until', 'until') + ' ' + when(i.ends_at) : null,
    done && i.resolved_at ? (discarded ? t('discarded_at', 'discarded') : t('resolved_at', 'resolved')) + ' ' + when(i.resolved_at)
      + by(i.resolved_by) : null,
    // For a discarded hub message, ends_at is when the provider resolved it too.
    discarded && i.ends_at ? t('provider_resolved', 'resolved by the service provider') + ' ' + when(i.ends_at) : null,
    ticketMeta(i),
    ...ext.meta.flatMap((fn) => fn(i) || []),
  ].filter(Boolean).map((p) => (typeof p === 'string' ? h('span', {}, p) : p));
  const act = (label, cls, fn) => h('button', { type: 'button', class: 'btn small ' + cls, onclick: fn }, label);
  const acts = [];
  if (draft) acts.push(act(t('publish', 'Publish'), '', () => change('POST', i.id + '/publish')),
                       act(t('discard', 'Discard'), 'ghost', () => change('DELETE', i.id)));
  else if (!done) acts.push(act(t('resolve', 'Resolve'), '', () => change('POST', i.id + '/resolve')));
  return h('div', { class: 'item ' + (draft ? 'draft' : discarded ? 'done discarded ' + i.severity + (i.ends_at ? ' over' : '') : done ? 'done ' + i.severity : i.severity) },
    h('div', { class: 'body' }, h('strong', {}, i.title), i.message && i.message !== i.title ? h('p', {}, i.message) : null,
      h('div', { class: 'meta' }, metaLine)),
    h('div', { class: 'acts' }, acts));
}

async function change(method, path) {
  try { await api(method, '/api/admin/incidents/' + path); } catch (e) { alertError(e); }
  loadIncidents();
}

function alertError(e) {
  if (e.status === 401 || e.status === 403) return showLogin();
  console.error(e);
}

async function loadIncidents() {
  let data;
  try { data = await api('GET', '/api/admin/incidents'); } catch (e) { return alertError(e); }
  const part = (status) => data.incidents.filter((i) => i.status === status);
  const fill = (id, list) => $(id).replaceChildren(...(list.length
    ? list.map((i) => incidentRow(i, data.now)) : [h('div', { class: 'empty' }, t('nothing', 'Nothing here.'))]));
  const drafts = part('draft');
  $('draftCard').hidden = drafts.length === 0;
  fill('drafts', drafts);
  fill('active', part('active'));
  // Declined hub messages stay in the history (marked black) so it shows what was turned down.
  fill('history', part('resolved').concat(part('discarded')).sort((a, b) => b.resolved_at - a.resolved_at).slice(0, 15));
  // Messages only for add-on recipients (e.g. other servers) are not on this server's desktops.
  const local = (i) => !(i.targets.length && i.targets.every((x) => extKind(x.split(':')[0])));
  onScreen = part('active').filter((i) => i.starts_at <= data.now && local(i))
    .concat(part('resolved').filter((i) => data.now - i.resolved_at < settings.resolved_seconds && local(i)));
  drawPreview();
}

const toUnix = (value) => (value ? Math.floor(new Date(value).getTime() / 1000) : null);

// ---- monitoring ------------------------------------------------------------------

async function loadMonitoring() {
  let data, m365 = null;
  try { data = await api('GET', '/api/admin/integrations'); } catch (e) { return alertError(e); }
  try { m365 = await api('GET', '/api/admin/m365'); } catch { /* status is informational */ }
  // Microsoft 365 service health is polled by the server and shown with its integration.
  const m365Line = (g) => {
    if (!m365 || !m365.configured || g.name.toLowerCase() !== (m365.integration || '').toLowerCase()) return null;
    if (m365.last_error) return h('span', { style: 'color: var(--red)' }, t('m365_error', 'Microsoft 365:') + ' ' + m365.last_error);
    if (!m365.last_ok) return t('m365_connecting', 'Microsoft 365: connecting…');
    return fill(t('m365_ok', 'Microsoft 365 checked {when}, {count} open issue(s)'), { when: when(m365.last_ok), count: m365.open.length });
  };
  const del = (path, id) => h('button', { type: 'button', class: 'btn small danger', onclick: async () => {
    if (!confirm(t('confirm_delete', 'Delete?'))) return;
    try { await api('DELETE', path + id); } catch (e) { alertError(e); }
    loadMonitoring();
  } }, t('delete', 'Delete'));
  $('integrations').replaceChildren(...(data.integrations.length ? data.integrations.map((g) =>
    h('div', { class: 'item' },
      h('div', { class: 'body' }, h('strong', {}, g.name),
        h('div', { class: 'meta' }, [t('unmatched_' + g.unmatched, g.unmatched),
          g.delay_seconds ? g.delay_seconds + ' s ' + t('delay_short', 'delay') : null,
          g.last_seen ? t('last_seen', 'last event') + ' ' + when(g.last_seen) + ' (' + g.events + ')' : t('never_seen', 'no events yet'),
          m365Line(g),
        ].filter(Boolean).map((p) => (typeof p === 'string' ? h('span', {}, p) : p)))),
      h('div', { class: 'acts' }, del('/api/admin/integrations/', g.id))))
    : [h('div', { class: 'empty' }, t('no_integrations', 'No integration yet.'))]));
  $('rules').replaceChildren(...(data.rules.length ? data.rules.map((r) =>
    h('div', { class: 'item ' + (r.severity || '') },
      h('div', { class: 'body' }, h('strong', {}, h('code', {}, r.pattern), '  →  ' + r.title), r.message ? h('p', {}, r.message) : null,
        h('div', { class: 'meta' }, r.targets.length ? r.targets.map(targetLabel).join(', ') : everyone())),
      h('div', { class: 'acts' }, del('/api/admin/rules/', r.id))))
    : [h('div', { class: 'empty' }, t('no_rules', 'No rules yet.'))]));
}

// ---- computers -------------------------------------------------------------------

// Compares dotted version numbers; unknown versions sort lowest.
function newer(a, b) {
  const pa = (a || '0').split('.').map(Number), pb = (b || '0').split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) > (pb[i] || 0);
  }
  return false;
}

async function loadComputers() {
  let d;
  try { d = await api('GET', '/api/admin/computers'); } catch (e) { return alertError(e); }
  const latest = d.computers.reduce((best, c) => (newer(c.version, best) ? c.version : best), '');
  const connected = d.computers.filter((c) => d.now - c.last_seen < 90).length;
  $('cCount').textContent = d.computers.length + ' ' + t('computers_30d', 'computers in the last 30 days') + ' \u00b7 '
    + connected + ' ' + t('connected_now', 'connected right now') + (latest ? ' \u00b7 ' + t('newest_client', 'newest client') + ' ' + latest : '');
  const filter = $('cFilter').value.trim().toLowerCase();
  const shown = d.computers.filter((c) => !filter || (c.computer + ' ' + c.user + ' ' + c.version).toLowerCase().includes(filter));
  $('computers').replaceChildren(...(shown.length ? shown.slice(0, 500).map((c) => {
    const online = d.now - c.last_seen < 90;
    const outdated = c.version && latest && newer(latest, c.version);
    return h('tr', {},
      h('td', {}, h('span', { class: 'dot ' + (online ? 'on' : '') }), c.computer.toUpperCase()),
      h('td', {}, c.user || '\u2013'),
      h('td', { class: outdated ? 'old' : '' }, c.version || t('version_unknown', 'unknown')),
      h('td', {}, online ? t('connected', 'connected') : when(c.last_seen)));
  }) : [h('tr', {}, h('td', { colspan: '4', class: 'empty' }, t('no_computers', 'No computer has connected yet.')))]));
}

async function loadDeploy() {
  loadComputers();
  let d;
  try { d = await api('GET', '/api/admin/deploy'); } catch (e) { return alertError(e); }
  $('dMsi').hidden = !d.has_msi;
  $('dNoMsi').hidden = d.has_msi;
  $('dPackage').href = '/api/admin/client-package.zip?server=' + encodeURIComponent(location.origin);
  $('dInstall').textContent = 'msiexec /i Statusbar.msi SERVERURL=' + location.origin + ' CLIENTTOKEN=' + d.client_token + ' /qn';
  $('dUninstall').textContent = 'msiexec /x Statusbar.msi /qn';
  $('dScript').textContent = ".\\install-client.ps1 -ExePath .\\statusbar-client.exe -ServerUrl " + location.origin + " -Token '" + d.client_token + "'";
}

// ---- settings --------------------------------------------------------------------

const FIELDS = { sName: 'name', sSubtitle: 'subtitle', sColor: 'color', sLanguage: 'language', sPosition: 'position',
  sResolved: 'resolved_seconds', sHeight: 'height_px', sInitialHeight: 'initial_height_px', sInitialSeconds: 'initial_seconds',
  sRdpMode: 'rdp_mode', sHubMessages: 'hub_messages', sLabelError: 'label_error', sLabelWarning: 'label_warning', sLabelInfo: 'label_info', sLabelResolved: 'label_resolved', sLabelAlarm: 'label_alarm',
  sAlarmHotkey: 'alarm_hotkey', sAlarmTitle: 'alarm_title', sAlarmMessage: 'alarm_message',
  sAlarmCooldown: 'alarm_cooldown_seconds', sAlarmMinutes: 'alarm_minutes' };

// ---- notification groups ---------------------------------------------------------------

let editingTeam = null;

async function refreshDirectory() {
  try { directory = await api('GET', '/api/admin/targets'); } catch { return; }
  for (const p of Object.values(pickers)) p.refresh();
}

async function loadTeams() {
  let d;
  try { d = await api('GET', '/api/admin/teams'); } catch (e) { return alertError(e); }
  $('teams').replaceChildren(...(d.teams.length ? d.teams.map((team) =>
    h('div', { class: 'item' },
      h('div', { class: 'body' }, h('strong', {}, team.name),
        h('div', { class: 'meta' }, team.targets.map((x) => h('span', {}, targetLabel(x))))),
      h('div', { class: 'acts' },
        h('button', { type: 'button', class: 'btn small ghost', onclick: () => {
          editingTeam = team.id; $('tName').value = team.name; pickers.team.set(team.targets);
          $('teamSave').textContent = t('save', 'Save'); $('teamCancel').hidden = false; $('tName').focus();
        } }, t('edit', 'Edit')),
        h('button', { type: 'button', class: 'btn small danger', onclick: async () => {
          if (!confirm(t('confirm_delete', 'Delete?'))) return;
          try { await api('DELETE', '/api/admin/teams/' + team.id); } catch (e) { alertError(e); }
          loadTeams(); refreshDirectory();
        } }, t('delete', 'Delete')))))
    : [h('div', { class: 'empty' }, t('no_teams', 'No notification group yet.'))]));
}

function resetTeamForm() {
  editingTeam = null; $('teamForm').reset(); pickers.team.clear();
  $('teamSave').textContent = t('create', 'Create'); $('teamCancel').hidden = true;
}

async function loadSettings() {
  try { settings = await api('GET', '/api/admin/settings'); } catch (e) { return alertError(e); }
  $('sLanguage').replaceChildren(...(meta.languages || ['en']).map((l) => h('option', { value: l }, l)));
  for (const [id, key] of Object.entries(FIELDS)) $(id).value = settings[key];
  $('sAllMonitors').checked = settings.all_monitors;
  $('sFullscreen').checked = settings.over_fullscreen;
  $('sAlarmEnabled').checked = settings.alarm_enabled;
  pickers.alarm.set(settings.alarm_targets || []);
  loadSounds();
  $('logoRemove').hidden = !meta.has_logo;
  loadTeams();
  $('sHubBox').hidden = !meta.uplink;
  if (meta.uplink) {
    try {
      const u = await api('GET', '/api/admin/uplink');
      $('sHubStatus').textContent = u.last_error
        ? t('hub_error', 'Not connected to the service provider:') + ' ' + u.last_error
        : u.last_ok ? t('hub_ok', 'Connected to your service provider, last contact') + ' ' + when(u.last_ok)
        : t('hub_connecting', 'Connecting to your service provider…');
    } catch { /* status is informational */ }
  }
  loadLicense();
  loadTicketing();
  drawPreview();
}

// ---- ticket system ---------------------------------------------------------------------

let ticketing = null;
// Per system: [locale key, label, example] for the fields that differ; null hides a field.
const TK_SYSTEMS = {
  webhook: { url: ['tk_url_webhook', 'Webhook URL', 'https://tickets.example.com/hooks/statusbar'],
    auth: ['tk_auth_webhook', 'Optional. A token without user name is sent as "Authorization: Bearer …", with a user name as HTTP Basic.'] },
  zammad: { url: ['tk_url', 'Address of the ticket system', 'https://zammad.example.com'],
    auth: ['tk_auth_zammad', 'An access token (Profile → Token Access, permission ticket.agent) with an empty user name – or user name and password.'],
    target: ['tk_target_group', 'Group', 'Users'], customer: ['tk_customer_email', 'Customer (e-mail, created if missing)', 'it@example.com'],
    close: ['tk_close_state', 'State when closing', 'closed'] },
  glpi: { url: ['tk_url', 'Address of the ticket system', 'https://glpi.example.com'], app: true,
    auth: ['tk_auth_glpi', 'The API token of a GLPI user (My settings → Remote access keys) with an empty user name – or user name and password. The legacy REST API must be enabled (Setup → General → API).'],
    target: ['tk_target_category', 'ITIL category ID (optional)', '12'],
    close: ['tk_close_glpi', 'Status when closing: 5 solved or 6 closed', '5'] },
  znuny: { url: ['tk_url_znuny', 'Address up to /otrs', 'https://helpdesk.example.com/otrs'],
    auth: ['tk_auth_znuny', 'Agent login and password. Needs the web service "Statusbar" from integrations/znuny-otrs.'],
    target: ['tk_target_queue', 'Queue', 'Raw'], customer: ['tk_customer_user', 'Customer user (login)', 'statusbar'],
    close: ['tk_close_state', 'State when closing', 'closed successful'] },
  jira: { url: ['tk_url', 'Address of the ticket system', 'https://example.atlassian.net'], untested: true,
    auth: ['tk_auth_jira', 'Cloud: e-mail address and API token. Data Center: a personal access token with an empty user name.'],
    target: ['tk_target_jira', 'Service desk ID / request type ID', '1/10'],
    customer: ['tk_customer_jira', 'On behalf of (account ID or e-mail, optional)', ''],
    close: ['tk_close_jira', 'Transition ID for closing (empty: comment only)', '761'] },
  servicenow: { url: ['tk_url', 'Address of the ticket system', 'https://example.service-now.com'], untested: true,
    auth: ['tk_auth_servicenow', 'User name and password of an integration user (roles itil and rest_api_explorer or web_service_admin) – or an OAuth token with an empty user name.'],
    target: ['tk_target_assignment', 'Assignment group (sys_id, optional)', ''],
    customer: ['tk_customer_caller', 'Caller (sys_id, optional)', ''],
    close: ['tk_close_state', 'State when closing', '6'] },
  topdesk: { url: ['tk_url', 'Address of the ticket system', 'https://example.topdesk.net'], untested: true,
    auth: ['tk_auth_topdesk', 'Login name of an operator and an application password (My settings → Application passwords).'],
    target: ['tk_target_category_name', 'Category (name, optional)', ''],
    customer: ['tk_customer_topdesk', 'Caller name (default: Statusbar)', 'Statusbar'],
    close: ['tk_close_topdesk', 'Processing status when closing (optional)', ''] },
};
const TK_PRIOS = { error: 'tkPrioError', warning: 'tkPrioWarning', info: 'tkPrioInfo', alarm: 'tkPrioAlarm' };

function drawTicketSystem() {
  const sys = $('tkSystem').value;
  const spec = TK_SYSTEMS[sys] || TK_SYSTEMS.webhook;
  const field = (box, label, input, def) => {
    if (box) $(box).hidden = !def;
    if (!def) return;
    $(label).textContent = t(def[0], def[1]);
    $(input).placeholder = def[2] || '';
  };
  field(null, 'tkUrlLabel', 'tkUrl', spec.url);
  field('tkTargetBox', 'tkTargetLabel', 'tkTarget', spec.target);
  field('tkCustomerBox', 'tkCustomerLabel', 'tkCustomer', spec.customer);
  field('tkCloseBox', 'tkCloseLabel', 'tkClose', spec.close);
  $('tkAppBox').hidden = !spec.app;
  $('tkAuthHint').textContent = t(spec.auth[0], spec.auth[1]);
  $('tkSystemHint').textContent = spec.untested
    ? t('tk_untested', 'Built from the vendor documentation, not tested against a live instance. Try it with "Create test ticket" first.') : '';
  const webhook = sys === 'webhook';
  $('tkWebhookBox').hidden = !webhook;
  const defaults = ticketing && ticketing.defaults[sys];
  for (const [kind, id] of Object.entries(TK_PRIOS)) $(id).placeholder = (defaults && defaults.priorities[kind]) || '–';
  if (defaults && spec.close && defaults.close_status) $('tkClose').placeholder = defaults.close_status;
}

function drawTicketStatus() {
  const d = ticketing;
  const c = d.config;
  const parts = [];
  if (!c.enabled) parts.push(t('tk_off', 'Switched off.'));
  if (d.last_ok) parts.push(fill(t('tk_last_ok', 'Last success {when} ({what})'), { when: when(d.last_ok), what: d.last_ok_what || '' }));
  parts.push(fill(t('tk_pending', '{n} waiting'), { n: d.pending }));
  if (d.failed) parts.push(fill(t('tk_failed', '{n} failed'), { n: d.failed }));
  $('tkStatus').replaceChildren(parts.join(' · '),
    ...(d.last_error ? [h('br'), h('span', { style: 'color: var(--red)' }, fill(t('tk_last_error', 'Last error {when}:'), { when: when(d.last_error_at) }) + ' ' + d.last_error)] : []));
  $('tkRetry').hidden = !d.failed;
  const action = (a) => t('tk_action_' + a, a);
  $('tkRecent').replaceChildren(...(d.recent.length ? [h('div', { class: 'tk-recent' }, ...d.recent.slice(0, 8).map((e) =>
    h('div', { class: e.state === 'failed' ? 'bad' : '' }, when(e.created_at) + ' · ' + (e.title || e.incident_id) + ' · ' + action(e.action) + ' · '
      + t('tk_state_' + e.state, e.state) + (e.attempts ? ' (' + e.attempts + ')' : '') + (e.last_error ? ' – ' + e.last_error : ''))))] : []));
}

async function loadTicketing() {
  try { ticketing = await api('GET', '/api/admin/ticketing'); } catch { return; }
  const c = ticketing.config;
  $('tkEnabled').checked = c.enabled;
  $('tkSystem').value = c.system;
  $('tkUrl').value = c.url; $('tkUser').value = c.username; $('tkSecret').value = ''; $('tkApp').value = '';
  $('tkTarget').value = c.target; $('tkCustomer').value = c.customer; $('tkClose').value = c.close_status;
  $('tkStatusbarUrl').value = c.statusbar_url; $('tkStatusbarUrl').placeholder = location.origin;
  $('tkSecret').placeholder = c.secret_set ? t('tk_secret_set', 'set – leave empty to keep it') : '';
  $('tkApp').placeholder = c.app_token_set ? t('tk_secret_set', 'set – leave empty to keep it') : '';
  $('tkClearBox').hidden = !c.secret_set; $('tkClear').checked = false;
  $('tkClearAppBox').hidden = !c.app_token_set; $('tkClearApp').checked = false;
  $('tkOnPublish').checked = c.on_publish; $('tkOnAlarm').checked = c.on_alarm; $('tkOnResolve').checked = c.on_resolve;
  for (const src of ['manual', 'monitoring', 'alarm', 'hub']) $('tkSrc-' + src).checked = c.sources.includes(src);
  $('tkResolveMode').value = c.resolve_mode;
  const tpl = ticketing.templates;
  $('tkTitleT').value = c.title_template; $('tkTitleT').placeholder = tpl.title;
  $('tkBodyT').value = c.body_template; $('tkBodyT').placeholder = tpl.body;
  $('tkNoteT').value = c.note_template; $('tkNoteT').placeholder = tpl.note;
  $('tkWebhookT').value = c.webhook_template; $('tkWebhookT').placeholder = tpl.webhook;
  for (const [kind, id] of Object.entries(TK_PRIOS)) $(id).value = c.priorities[kind] || '';
  $('tkPlaceholders').textContent = t('tk_placeholders', 'Placeholders:') + ' ' + ticketing.placeholders.map((p) => '{' + p + '}').join(' ')
    + '. ' + t('tk_placeholders_hint', 'Empty fields use the default shown in grey. A line whose placeholders are all empty is left out.');
  drawTicketSystem();
  drawTicketStatus();
}

async function saveTicketing(ev) {
  ev.preventDefault();
  const msg = (ev.submitter && ev.submitter.dataset.msg) || 'tkMsg';
  for (const other of ['tkMsg', 'tkMsg2']) if (other !== msg) say(other, '');
  const priorities = {};
  for (const [kind, id] of Object.entries(TK_PRIOS)) priorities[kind] = $(id).value;
  const body = {
    enabled: $('tkEnabled').checked, system: $('tkSystem').value, url: $('tkUrl').value, username: $('tkUser').value,
    secret: $('tkSecret').value, app_token: $('tkApp').value, clear_secret: $('tkClear').checked, clear_app_token: $('tkClearApp').checked,
    target: $('tkTarget').value, customer: $('tkCustomer').value, close_status: $('tkClose').value,
    on_publish: $('tkOnPublish').checked, on_alarm: $('tkOnAlarm').checked, on_resolve: $('tkOnResolve').checked,
    sources: ['manual', 'monitoring', 'alarm', 'hub'].filter((src) => $('tkSrc-' + src).checked),
    resolve_mode: $('tkResolveMode').value, title_template: $('tkTitleT').value, body_template: $('tkBodyT').value,
    note_template: $('tkNoteT').value, webhook_template: $('tkWebhookT').value, priorities,
    statusbar_url: $('tkStatusbarUrl').value,
  };
  try {
    await api('PUT', '/api/admin/ticketing', body);
    await loadTicketing();
    say(msg, t('saved', 'Saved.'), true);
  } catch (e) { say(msg, e.message); }
}

async function testTicket() {
  say('tkMsg', t('tk_testing', 'Opening a test ticket…'), true);
  try {
    const r = await api('POST', '/api/admin/ticketing/test');
    if (r.ok) {
      const label = fill(t('tk_test_ok', 'Test ticket {ticket} opened.'), { ticket: r.reference || r.ticket_id || '' });
      $('tkMsg').className = 'msg good';
      $('tkMsg').replaceChildren(label, ...(r.url ? [' ', h('a', { href: r.url, target: '_blank', rel: 'noopener noreferrer' }, t('tk_open', 'Open'))] : []));
    } else say('tkMsg', t('tk_test_failed', 'Test ticket failed:') + ' ' + r.error);
  } catch (e) { say('tkMsg', e.message); }
  loadTicketing();
}

// ---- sounds ------------------------------------------------------------------------

let soundInfo = {};
const SOUND_NAMES = { SystemAsterisk: 'Asterisk', SystemExclamation: 'Exclamation', SystemHand: 'Critical Stop',
  SystemNotification: 'System Notification', SystemDefault: 'Default Beep', 'Notification.Default': 'Notification',
  'Notification.IM': 'Instant Message', 'Notification.Mail': 'New Mail', 'Notification.Reminder': 'Reminder',
  'Notification.SMS': 'New Text Message' };

async function loadSounds() {
  try { soundInfo = await api('GET', '/api/admin/sounds'); } catch { return; }
  const kindName = (k) => (k === 'resolved' ? settings.label_resolved || t('sev_resolved', 'All-clear')
    : settings['label_' + k] || t('sev_' + k, k));
  $('sSounds').replaceChildren(...soundInfo.kinds.map((kind) => {
    const choice = h('select', { id: 'sound-' + kind, 'aria-label': kindName(kind) },
      h('option', { value: '' }, t('sound_none', 'No sound')),
      ...soundInfo.aliases.map((a) => h('option', { value: a }, 'Windows: ' + (SOUND_NAMES[a] || a))),
      h('option', { value: 'custom' }, t('sound_custom', 'Own sound (WAV)')));
    choice.value = settings['sound_' + kind] || '';
    const file = h('input', { type: 'file', id: 'soundFile-' + kind, accept: 'audio/wav,.wav' });
    const has = soundInfo.uploaded[kind];
    const listen = h('button', { type: 'button', class: 'btn small ghost', onclick: () => new Audio('/api/sounds/' + kind + '?' + has).play() },
      t('sound_play', 'Listen'));
    const drop = h('button', { type: 'button', class: 'btn small ghost', onclick: async () => {
      try { await api('DELETE', '/api/admin/sounds/' + kind); } catch (e) { say('soundsMsg', e.message); }
      loadSounds();
    } }, t('sound_remove', 'Remove file'));
    const own = h('div', {}, h('div', { class: 'row' }, file, ...(has ? [listen, drop] : [])),
      has ? null : h('p', { class: 'hint' }, t('sound_missing', 'No file uploaded yet – this kind stays silent until you add one.')));
    const toggle = () => { own.hidden = choice.value !== 'custom'; };
    choice.addEventListener('change', toggle); toggle();
    return h('div', { class: 'sound' }, h('label', { for: 'sound-' + kind }, h('span', { class: 'dot ' + kind }), kindName(kind)),
      choice, own);
  }));
}

// ---- license: only ever a notice, nothing is blocked ----------------------------------

const fill = (text, values) => text.replace(/\{(\w+)\}/g, (m, k) => (k in values ? values[k] : m));

async function loadLicense() {
  let l;
  try { l = await api('GET', '/api/admin/license'); } catch { return; }
  const day = (ts) => new Date(ts * 1000).toLocaleDateString(meta.language || undefined);
  const v = { n: l.computers, free: l.free_limit, max: l.max_computers, licensee: l.licensee || '',
    until: l.good_until ? day(l.good_until) : '', since: l.last_ok ? day(l.last_ok) : '',
    state: t('license_state_' + l.state, l.state) };
  const notes = {
    unlicensed: l.kind === 'hub'
      ? t('license_note_hub', 'A hub needs a hub license. Enter it as LICENSE_KEY.')
      : t('license_note_unlicensed', '{n} computers use Statusbar. Up to {free} it is free; for more, please enter a license as LICENSE_KEY.'),
    over_limit: t('license_note_over', 'The license covers {max} computers, {n} are connected.'),
    invalid: t('license_note_invalid', 'The license key does not work: {state}.'),
    stale: t('license_note_stale', 'The license portal has not been reachable since {since}, so the license could not be confirmed.'),
    unverified: t('license_note_unverified', 'The license portal cannot be reached, the license has not been confirmed yet.'),
    offline: t('license_note_offline', 'The license portal cannot be reached right now. The license stays valid until {until}.'),
  };
  const note = $('licenseNote');
  note.hidden = !l.notice;
  if (l.notice) note.textContent = fill(notes[l.notice] + ' ' + t('license_never_blocks', 'Messages keep working without restriction.'), v);
  let status;
  if (l.state === 'hub_connected') status = t('license_hub', 'Covered by your service provider.');
  else if (l.state === 'unlicensed') status = fill(t('license_none', 'No license – free up to {free} computers ({n} connected).'), v);
  else if (l.state === 'valid') {
    const parts = [fill(t('license_valid', 'Licensed to {licensee}'), v)];
    parts.push(l.max_computers ? fill(t('license_max', '{n} of {max} computers'), v) : fill(t('license_unlimited', '{n} computers, no limit'), v));
    if (l.valid_until) parts.push(t('license_until', 'valid until') + ' ' + day(l.valid_until));
    if (l.free) parts.push(t('license_free', 'free of charge'));
    if (l.last_ok) parts.push(t('license_checked', 'checked') + ' ' + when(l.last_ok));
    status = parts.join(' · ');
  } else status = fill(t('license_problem', 'License: {state}'), v);
  if (l.last_error) status += ' – ' + l.last_error;
  $('sLicense').textContent = status;
}

async function saveSettings(ev) {
  ev.preventDefault();
  // Both panels have a save button; answer next to the one that was used.
  const msg = (ev.submitter && ev.submitter.dataset.msg) || 'settingsMsg';
  for (const other of ['brandingMsg', 'settingsMsg', 'alarmMsg', 'soundsMsg']) if (other !== msg) say(other, '');
  const next = { all_monitors: $('sAllMonitors').checked, over_fullscreen: $('sFullscreen').checked,
    alarm_enabled: $('sAlarmEnabled').checked, alarm_targets: pickers.alarm.get() };
  for (const [id, key] of Object.entries(FIELDS)) next[key] = $(id).type === 'number' ? Number($(id).value) : $(id).value;
  for (const kind of soundInfo.kinds || []) next['sound_' + kind] = $('sound-' + kind).value;
  try {
    for (const kind of soundInfo.kinds || []) {
      const upload = $('soundFile-' + kind).files[0];
      if (upload) await api('PUT', '/api/admin/sounds/' + kind, upload);
    }
    const file = $('sLogo').files[0];
    if (file) await api('PUT', '/api/admin/logo', file);
    await api('PUT', '/api/admin/settings', next);
    $('sLogo').value = '';
    await boot(true);
    say(msg, t('saved', 'Saved.'), true);
  } catch (e) { say(msg, e.message); }
}

// ---- shell -----------------------------------------------------------------------

function applyBrand() {
  document.title = meta.name;
  $('brandName').textContent = meta.name;
  $('brandSub').textContent = meta.subtitle;
  $('version').textContent = 'v' + meta.version;
  document.documentElement.style.setProperty('--brand', meta.color);
  document.documentElement.lang = meta.language;
  $('logo').hidden = !meta.has_logo;
  if (meta.has_logo) $('logo').src = '/api/logo?' + Date.now();
  document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n, el.textContent); });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => { el.placeholder = t(el.dataset.i18nPlaceholder, el.placeholder); });
  const standalone = meta.mode !== 'directory';
  $('userField').hidden = standalone;
  $('passLabel').textContent = standalone ? t('admin_token', 'Administrator token') : t('password', 'Password');
  $('loginHint').textContent = standalone ? t('login_hint_standalone', 'Use the ADMIN_TOKEN from the server configuration.')
    : t('login_hint_directory', 'Use your directory account. Only members of the administrator group may sign in.');
}

function showLogin() {
  clearInterval(timer);
  for (const id of TABS) $('tab-' + id).hidden = true;
  $('side').hidden = true;
  $('login').hidden = false;
}

function showTab(name) {
  for (const b of $('nav').children) b.classList.toggle('on', b.dataset.tab === name);
  for (const id of TABS) $('tab-' + id).hidden = id !== name;
  clearInterval(timer);
  history.replaceState(null, '', '#' + name);
  if (name === 'incidents') { loadIncidents(); timer = setInterval(loadIncidents, 5000); }
  if (name === 'monitoring') loadMonitoring();
  if (name === 'settings') loadSettings();
  if (name === 'deploy') { loadDeploy(); timer = setInterval(loadComputers, 15000); }
  const added = ext.loaders[name];
  if (added) { added.load(); if (added.every) timer = setInterval(added.load, added.every * 1000); }
}

async function enter() {
  $('login').hidden = true;
  $('side').hidden = false;
  try { directory = await api('GET', '/api/admin/targets'); } catch (e) { /* host targets still work */ }
  for (const p of Object.values(pickers)) p.refresh();
  await loadSettings();
  const wanted = location.hash.slice(1);
  showTab(TABS.includes(wanted) ? wanted : 'incidents');
}

async function boot(keepTab) {
  meta = await api('GET', '/api/meta');
  try { strings = await api('GET', '/api/locales/' + meta.language); } catch { strings = {}; }
  if (meta.script && !window.Statusbar.loaded) {
    window.Statusbar.loaded = true;
    await new Promise((done) => {
      const s = document.createElement('script');
      s.src = '/ext.js'; s.onload = done; s.onerror = done;
      document.head.append(s);
    });
  }
  applyBrand();
  drawSeverity();
  if (keepTab) { for (const p of Object.values(pickers)) p.refresh(); return loadSettings(); }
  try { await api('GET', '/api/me'); await enter(); } catch { showLogin(); }
}

document.addEventListener('DOMContentLoaded', () => {
  pickers = { incident: picker($('iTargets')), integration: picker($('gTargets')), rule: picker($('rTargets')),
    team: picker($('tTargets'), true), alarm: picker($('sAlarmTargets')) };
  $('nav').addEventListener('click', (e) => { if (e.target.dataset.tab) showTab(e.target.dataset.tab); });
  $('iTitle').addEventListener('input', drawPreview);
  $('iMessage').addEventListener('input', drawPreview);

  $('loginForm').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    say('loginMsg', '');
    try {
      await api('POST', '/api/login', { username: $('user').value, password: $('pass').value });
      $('pass').value = '';
      await enter();
    } catch (e) { say('loginMsg', t('login_failed', 'Sign-in failed.')); }
  });
  $('logout').addEventListener('click', async () => { try { await api('POST', '/api/logout'); } catch { /* ignore */ } showLogin(); });

  $('incidentForm').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    try {
      await api('POST', '/api/admin/incidents', { title: $('iTitle').value, message: $('iMessage').value, severity,
        targets: pickers.incident.get(), starts_at: toUnix($('iStart').value), ends_at: toUnix($('iEnd').value) });
      ev.target.reset(); pickers.incident.clear(); drawPreview();
      say('incidentMsg', t('published', 'Published.'), true);
      loadIncidents();
    } catch (e) { say('incidentMsg', e.message); }
  });

  $('integrationForm').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    try {
      const made = await api('POST', '/api/admin/integrations', { name: $('gName').value, unmatched: $('gUnmatched').value,
        delay_seconds: Number($('gDelay').value) || 0, targets: pickers.integration.get() });
      const url = location.origin + '/api/hooks/' + made.token;
      $('tokenUrl').textContent = url;
      $('tokenCurl').textContent = "curl -X POST '" + url + "' -H 'Content-Type: application/json' \\\n  -d '{\"status\":\"problem\",\"host\":\"mail01\",\"service\":\"SMTP\",\"message\":\"test\"}'";
      $('tokenBox').hidden = false;
      ev.target.reset(); pickers.integration.clear(); say('integrationMsg', '');
      loadMonitoring();
    } catch (e) { say('integrationMsg', e.message); }
  });

  $('ruleForm').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    try {
      await api('POST', '/api/admin/rules', { pattern: $('rPattern').value, title: $('rTitle').value, message: $('rMessage').value,
        severity: $('rSeverity').value, targets: pickers.rule.get() });
      ev.target.reset(); pickers.rule.clear(); say('ruleMsg', '');
      loadMonitoring();
    } catch (e) { say('ruleMsg', e.message); }
  });

  document.addEventListener('click', async (e) => {
    const id = e.target.dataset && e.target.dataset.copy;
    if (!id) return;
    try { await navigator.clipboard.writeText($(id).textContent); e.target.textContent = t('copied', 'Copied'); } catch { /* clipboard blocked */ }
  });
  $('teamForm').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const body = { name: $('tName').value, targets: pickers.team.get() };
    try {
      if (editingTeam) await api('PUT', '/api/admin/teams/' + editingTeam, body);
      else await api('POST', '/api/admin/teams', body);
      resetTeamForm(); say('teamMsg', t('saved', 'Saved.'), true);
      loadTeams(); refreshDirectory();
    } catch (e) { say('teamMsg', e.status === 409 ? t('team_exists', 'A notification group with this name exists.') : e.message); }
  });
  $('teamCancel').addEventListener('click', resetTeamForm);
  $('cFilter').addEventListener('input', loadComputers);
  $('settingsForm').addEventListener('submit', saveSettings);
  $('ticketForm').addEventListener('submit', saveTicketing);
  $('tkSystem').addEventListener('change', drawTicketSystem);
  $('tkTest').addEventListener('click', testTicket);
  $('tkRetry').addEventListener('click', async () => {
    try { await api('POST', '/api/admin/ticketing/retry'); } catch (e) { say('tkMsg', e.message); }
    setTimeout(loadTicketing, 1500);
  });
  $('logoRemove').addEventListener('click', async () => {
    try { await api('DELETE', '/api/admin/logo'); await boot(true); } catch (e) { say('settingsMsg', e.message); }
  });

  boot().catch((e) => { document.body.append(h('pre', {}, String(e))); });
});

// ---- add-on API -------------------------------------------------------------------------
// An add-on served as /ext.js calls Statusbar.extend({...}):
//   tab:   { id, label, render(section), load(), every }   adds a page to the navigation
//   kinds: [{ kind, label, options(), placeholder, labelFor(value) }]   extra recipient kinds
//   incidentMeta(incident): [text, ...]   extra details under a message
window.Statusbar = {
  h, t, api, when, say, $,
  meta: () => meta,
  directory: () => directory,
  refreshDirectory: () => refreshDirectory(),
  extend(spec) {
    if (spec.kinds) ext.kinds.push(...spec.kinds);
    if (spec.incidentMeta) ext.meta.push(spec.incidentMeta);
    if (spec.tab) {
      const tab = spec.tab;
      const section = h('section', { id: 'tab-' + tab.id, hidden: '' });
      document.querySelector('main').insertBefore(section, $('tab-deploy'));
      $('nav').insertBefore(h('button', { 'data-tab': tab.id }, tab.label), $('nav').querySelector('[data-tab="deploy"]'));
      TABS.push(tab.id);
      ext.loaders[tab.id] = tab;
      tab.render(section);
    }
    for (const p of Object.values(pickers)) p.refresh();
  },
};

