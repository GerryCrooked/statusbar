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

function targetLabel(target) {
  const i = target.indexOf(':');
  const kind = target.slice(0, i), value = target.slice(i + 1);
  const short = kind === 'group' || kind === 'ou' ? value.split(',')[0].replace(/^(cn|ou)=/i, '') : value;
  return t('kind_' + kind, kind) + ': ' + short;
}

function picker(host) {
  const state = [];
  const chips = h('div', { class: 'chips' });
  const kind = h('select', {});
  const value = h('input', { list: host.id + 'List', placeholder: 'PC-ACCOUNTING-*' });
  const list = h('datalist', { id: host.id + 'List' });
  const draw = () => {
    chips.replaceChildren(...(state.length
      ? state.map((target, i) => h('span', { class: 'chip', title: target }, targetLabel(target),
          h('button', { type: 'button', 'aria-label': 'remove', onclick: () => { state.splice(i, 1); draw(); } }, '×')))
      : [h('span', { class: 'chip' }, t('everyone', 'Everyone'))]));
  };
  const options = () => {
    const kinds = [['host', t('kind_host', 'Computer name')]];
    if (directory.directory) kinds.push(['group', t('kind_group', 'Group')], ['ou', t('kind_ou', 'OU')], ['user', t('kind_user', 'User')]);
    kind.replaceChildren(...kinds.map(([v, label]) => h('option', { value: v }, label)));
    fill();
  };
  const fill = () => {
    const source = { group: directory.groups, ou: directory.ous, user: directory.users }[kind.value] || [];
    list.replaceChildren(...source.slice(0, 2000).map((v) => h('option', { value: v })));
    value.placeholder = kind.value === 'host' ? 'PC-ACCOUNTING-*' : '';
  };
  const add = () => {
    const v = value.value.trim();
    if (!v) return;
    const target = kind.value + ':' + v;
    if (!state.includes(target)) state.push(target);
    value.value = '';
    draw();
  };
  kind.addEventListener('change', fill);
  value.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); add(); } });
  host.replaceChildren(
    h('div', { class: 'row' }, kind, value, h('button', { type: 'button', class: 'btn ghost small', onclick: add }, '+')),
    list, chips);
  options(); draw();
  return { get: () => state.slice(), clear: () => { state.length = 0; draw(); }, refresh: () => { options(); draw(); } };
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
    const i = onScreen[0];
    const kind = i.status === 'resolved' ? 'resolved' : i.severity;
    bar.className = 'bar ' + kind;
    bar.textContent = barText(settings['label_' + kind], i.title, i.message);
    $('stageTitle').textContent = onScreen.length === 1 ? t('stage_one', 'One message is on screen')
      : onScreen.length + ' ' + t('stage_many', 'messages are on screen');
    $('stageNote').textContent = t('stage_live_note', 'Desks show this bar right now. Several messages take turns.');
  } else {
    bar.hidden = true;
    $('stageTitle').textContent = t('stage_quiet', 'All quiet');
    $('stageNote').textContent = t('stage_quiet_note', 'No bar on any desk. Publish a message or let your monitoring do it.');
  }
}

function incidentRow(i, now) {
  const draft = i.status === 'draft', done = i.status === 'resolved';
  const waiting = i.status === 'active' && i.starts_at > now;
  const metaLine = [when(i.created_at),
    i.targets.length ? i.targets.map(targetLabel).join(', ') : t('everyone', 'Everyone'),
    i.source !== 'manual' ? t('via_monitoring', 'via monitoring') : null,
    waiting ? t('starts', 'Show from') + ' ' + when(i.starts_at) : null,
    i.ends_at && !done ? t('until', 'until') + ' ' + when(i.ends_at) : null,
    done && i.resolved_at ? t('resolved_at', 'resolved') + ' ' + when(i.resolved_at) : null,
  ].filter(Boolean).map((p) => h('span', {}, p));
  const act = (label, cls, fn) => h('button', { type: 'button', class: 'btn small ' + cls, onclick: fn }, label);
  const acts = [];
  if (draft) acts.push(act(t('publish', 'Publish'), '', () => change('POST', i.id + '/publish')),
                       act(t('discard', 'Discard'), 'ghost', () => change('DELETE', i.id)));
  else if (!done) acts.push(act(t('resolve', 'Resolve'), '', () => change('POST', i.id + '/resolve')));
  return h('div', { class: 'item ' + (draft ? 'draft' : done ? 'done' : i.severity) },
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
  fill('history', part('resolved').sort((a, b) => b.resolved_at - a.resolved_at).slice(0, 15));
  onScreen = part('active').filter((i) => i.starts_at <= data.now)
    .concat(part('resolved').filter((i) => data.now - i.resolved_at < settings.resolved_seconds));
  drawPreview();
}

const toUnix = (value) => (value ? Math.floor(new Date(value).getTime() / 1000) : null);

// ---- monitoring ------------------------------------------------------------------

async function loadMonitoring() {
  let data;
  try { data = await api('GET', '/api/admin/integrations'); } catch (e) { return alertError(e); }
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
        ].filter(Boolean).map((p) => h('span', {}, p)))),
      h('div', { class: 'acts' }, del('/api/admin/integrations/', g.id))))
    : [h('div', { class: 'empty' }, t('no_integrations', 'No integration yet.'))]));
  $('rules').replaceChildren(...(data.rules.length ? data.rules.map((r) =>
    h('div', { class: 'item ' + (r.severity || '') },
      h('div', { class: 'body' }, h('strong', {}, h('code', {}, r.pattern), '  →  ' + r.title), r.message ? h('p', {}, r.message) : null,
        h('div', { class: 'meta' }, r.targets.length ? r.targets.map(targetLabel).join(', ') : t('everyone', 'Everyone'))),
      h('div', { class: 'acts' }, del('/api/admin/rules/', r.id))))
    : [h('div', { class: 'empty' }, t('no_rules', 'No rules yet.'))]));
}

// ---- computers -------------------------------------------------------------------

async function loadDeploy() {
  let d;
  try { d = await api('GET', '/api/admin/deploy'); } catch (e) { return alertError(e); }
  $('dMsi').hidden = !d.has_msi;
  $('dNoMsi').hidden = d.has_msi;
  $('dInstall').textContent = 'msiexec /i Statusbar.msi SERVERURL=' + location.origin + ' CLIENTTOKEN=' + d.client_token + ' /qn';
  $('dUninstall').textContent = 'msiexec /x Statusbar.msi /qn';
  $('dScript').textContent = ".\\install-client.ps1 -ExePath .\\statusbar-client.exe -ServerUrl " + location.origin + " -Token '" + d.client_token + "'";
}

// ---- settings --------------------------------------------------------------------

const FIELDS = { sName: 'name', sSubtitle: 'subtitle', sColor: 'color', sLanguage: 'language', sPosition: 'position',
  sResolved: 'resolved_seconds', sHeight: 'height_px', sInitialHeight: 'initial_height_px', sInitialSeconds: 'initial_seconds',
  sLabelError: 'label_error', sLabelWarning: 'label_warning', sLabelInfo: 'label_info', sLabelResolved: 'label_resolved' };

async function loadSettings() {
  try { settings = await api('GET', '/api/admin/settings'); } catch (e) { return alertError(e); }
  $('sLanguage').replaceChildren(...(meta.languages || ['en']).map((l) => h('option', { value: l }, l)));
  for (const [id, key] of Object.entries(FIELDS)) $(id).value = settings[key];
  $('sAllMonitors').checked = settings.all_monitors;
  $('sFullscreen').checked = settings.over_fullscreen;
  $('logoRemove').hidden = !meta.has_logo;
  drawPreview();
}

async function saveSettings(ev) {
  ev.preventDefault();
  const next = { all_monitors: $('sAllMonitors').checked, over_fullscreen: $('sFullscreen').checked };
  for (const [id, key] of Object.entries(FIELDS)) next[key] = $(id).type === 'number' ? Number($(id).value) : $(id).value;
  try {
    const file = $('sLogo').files[0];
    if (file) await api('PUT', '/api/admin/logo', file);
    await api('PUT', '/api/admin/settings', next);
    $('sLogo').value = '';
    await boot(true);
    say('settingsMsg', t('saved', 'Saved.'), true);
  } catch (e) { say('settingsMsg', e.message); }
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
  if (name === 'deploy') loadDeploy();
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
  applyBrand();
  drawSeverity();
  if (keepTab) { for (const p of Object.values(pickers)) p.refresh(); return loadSettings(); }
  try { await api('GET', '/api/me'); await enter(); } catch { showLogin(); }
}

document.addEventListener('DOMContentLoaded', () => {
  pickers = { incident: picker($('iTargets')), integration: picker($('gTargets')), rule: picker($('rTargets')) };
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
  $('settingsForm').addEventListener('submit', saveSettings);
  $('logoRemove').addEventListener('click', async () => {
    try { await api('DELETE', '/api/admin/logo'); await boot(true); } catch (e) { say('settingsMsg', e.message); }
  });

  boot().catch((e) => { document.body.append(h('pre', {}, String(e))); });
});
