// FlexConnect — логика окна. Всё, что приходит из подписок (названия серверов и т. п.),
// вставляется только как текст, никогда как HTML.
'use strict';

const t = (...args) => FlyxI18n.t(...args);
let platformOS = 'Windows';
const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : window.FlyxAndroidInvoke || mockInvoke;
if (!TAURI && !window.FlyxAndroid) document.documentElement.classList.add('preview');

const RU_DOMAINS = FlyxRuServices.suffixes;
const MANUAL_ID = 'manual';

const $ = (id) => document.getElementById(id);

// Polling must not replace unchanged text nodes or invalidate glass surfaces.
function setText(id, text) {
  const node = $(id);
  if (node.textContent !== text) node.textContent = text;
}
function setProperty(node, key, value) {
  if (node[key] !== value) node[key] = value;
}

function el(tag, props, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props || {})) {
    if (key === 'class') node.className = value;
    else if (key === 'text') node.textContent = value;
    else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  for (const child of children) if (child) node.append(child);
  return node;
}

function icon(paths) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  for (const d of paths) {
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', d);
    svg.append(path);
  }
  return svg;
}

// ---------- состояние ----------

const state = {
  profiles: [],                      // {id, name, source, nodes, used, total, expire, updated}
  preferences: { theme: 'flyx', language: 'ru', autoConnect: false, startHidden: false },
  selected: null,                    // {profile, index}
  settings: { mode: 'tun', proxyPort: 2080, split: 'all', apps: [], ruDirect: false, domains: [] },
};

const live = {
  service: 'unknown',                // unknown | missing | ok
  tunnel: 'idle',                    // idle | connected | failed
  busy: false,
  server: null,
  since: null,
  error: null,
  appliedKey: null,                  // отпечаток настроек, с которыми подключились
  pings: new Map(),                  // "host:port" -> мс | null
  pinging: new Set(),                // "host:port", замер ещё идёт
  pingingProfiles: new Set(),        // id подписок, где нажат «пинг всех»
};

let saveTimer = 0;
function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => invoke('save_state', { state }).catch(() => {}), 300);
}

const providerNotice = (name) => /приложение не поддерживается|рекомендуемые приложения|app is not supported|application not supported|unsupported client/i.test(name || '');

function selectedNode() {
  if (!state.selected) return null;
  const profile = state.profiles.find((p) => p.id === state.selected.profile);
  return (profile && profile.nodes[state.selected.index]) || null;
}

function connectRequest() {
  const view = selectedNode();
  if (!view) return null;
  const s = state.settings;
  const apps = s.split === 'all' ? [] : s.apps;
  return {
    node: view.node,
    mode: s.mode,
    proxy_port: s.proxyPort,
    split: apps.length ? s.split : 'all',
    apps,
    direct_domains: [...(s.ruDirect ? RU_DOMAINS : []), ...s.domains],
  };
}

function requestKey(request = connectRequest()) {
  if (!request) return null;
  // Compare the effective connection, not server labels, JSON field order,
  // duplicate routing entries or a proxy port unused in TUN mode.
  const { name, ...node } = request.node;
  const sorted = (value) => Array.isArray(value) ? value.map(sorted)
    : value && typeof value === 'object'
      ? Object.fromEntries(Object.keys(value).sort().map(key => [key, sorted(value[key])]))
      : value;
  const list = (items) => [...new Set(items.map(item => item.trim().toLowerCase()))].sort();
  return JSON.stringify(sorted({
    ...request, node,
    proxy_port: request.mode === 'proxy' ? request.proxy_port : null,
    apps: list(request.apps), direct_domains: list(request.direct_domains),
  }));
}

// ---------- служба ----------

function applyResponse(response) {
  live.service = 'ok';
  live.tunnel = response.state;
  live.server = response.server || null;
  live.since = response.since || null;
  live.error = response.state === 'failed' ? response.error || t('failed') : null;
}

async function refreshStatus() {
  if (live.busy) return;
  try {
    applyResponse(await invoke('svc_status'));
  } catch (e) {
    live.service = 'missing';
    live.tunnel = 'idle';
    live.error = null;
  }
  renderHome();
}

async function connect() {
  const request = connectRequest();
  if (!request) { showPage('servers'); $('add-input').focus(); return; }
  const submittedKey = requestKey(request);
  live.busy = true;
  live.error = null;
  renderHome();
  try {
    const response = await invoke('svc_connect', { request });
    applyResponse(response);
    if (!response.ok) { live.tunnel = response.state === 'connected' ? 'connected' : 'failed'; live.error = response.error || t('failed'); }
    else live.appliedKey = submittedKey;
  } catch (e) {
    live.service = 'missing';
    live.tunnel = 'failed';
    live.error = String(e);
  }
  live.busy = false;
  renderHome();
}

async function disconnect() {
  live.busy = true;
  renderHome();
  try { applyResponse(await invoke('svc_disconnect')); } catch (e) { live.error = String(e); }
  live.appliedKey = null;
  live.busy = false;
  renderHome();
}

// ---------- экономия ресурсов ----------

// Проигрывание анимаций — почти вся нагрузка окна (около 16 % ядра против 0,4 % на паузе).
// Поэтому, пока окно Flyx открыто, но не активно, маскот, флаги и стикеры замирают
// на текущем кадре и оживают, как только в окно возвращаются.
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
function syncMascotPlayback() {
  const mascot = $('mascot');
  if (!mascot.getAttribute('src')) return;
  if (window.FlyxUnfocused || document.hidden || reducedMotion.matches) mascot.pause();
  else if (mascot.paused) mascot.play().catch(() => {});
}
function setUnfocused(value) {
  if (window.FlyxUnfocused === value) return;
  window.FlyxUnfocused = value;
  // флаги, стикеры и маскоты страниц пересчитывают своё состояние по этому событию
  window.dispatchEvent(new Event('flyx-background'));
  syncMascotPlayback();
}
window.addEventListener('blur', () => setUnfocused(true));
window.addEventListener('focus', () => setUnfocused(false));
document.addEventListener('visibilitychange', syncMascotPlayback);
reducedMotion.addEventListener('change', syncMascotPlayback);

// ---------- главная ----------

function formatElapsed(since) {
  const total = Math.max(0, Math.floor(Date.now() / 1000) - since);
  const pad = (n) => String(n).padStart(2, '0');
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor(total / 60) % 60)}:${pad(total % 60)}`;
}

function describeNode(view) {
  return [view.proto, view.security !== 'none' ? view.security : null, view.transport !== 'tcp' ? view.transport : null]
    .filter(Boolean).join(' · ');
}

function renderHome() {
  const visual = live.busy ? 'busy' : live.tunnel;
  setProperty(document.body.dataset, 'state', visual);
  const mascotSource = window.FlyxBackground ? '' : live.tunnel === 'connected' ? 'assets/connected.webm' : 'assets/disconnected.webm';
  if ($('mascot').getAttribute('src') !== mascotSource) {
    // Маскот — видео WebM с прозрачностью: анимированная картинка той же длины грузила
    // процессор на десятки процентов, видео декодируется ровно и дёшево.
    const mascot = $('mascot');
    if (mascotSource) mascot.src = mascotSource;
    else { mascot.pause(); mascot.removeAttribute('src'); mascot.load(); }
  }
  syncMascotPlayback();
  setProperty($('power'), 'disabled', live.busy);
  setText('power-label', live.busy ? t('wait') : live.tunnel === 'connected' ? t('disconnect') : live.tunnel === 'failed' ? t('retry') : t('connect'));
  FlyxStickers.set($('power-sticker'), live.busy ? 'loading' : live.tunnel === 'connected' ? 'disconnect' : 'connect');

  let title = t('idle');
  let sub = t('tapConnect');
  if (live.busy) { title = live.tunnel === 'connected' ? t('disconnecting') : t('connecting'); sub = t('moment'); }
  else if (live.tunnel === 'connected') { title = t('connected'); sub = live.since ? formatElapsed(live.since) : ''; }
  else if (live.tunnel === 'failed') { title = t('failed'); sub = t('tapRetry'); }
  else if (live.service === 'missing') { sub = t('serviceMissing'); }
  setText('status-title', title);
  setText('status-sub', sub);

  setProperty($('notice'), 'hidden', !live.error);
  setText('notice-text', live.error ? FlyxI18n.error(live.error) : '');

  const view = selectedNode();
  const shownName = live.tunnel === 'connected' && live.server ? live.server : view && view.name;
  setText('server-name', shownName ? FlyxCountries.label(shownName) : t('noServer'));
  FlyxCountries.set($('server-symbol'), shownName || '');
  setText('server-sub', view ? `${describeNode(view)}${view.via_xray ? ` · ${t('viaXray')}` : ''}` : t('addSource'));

  const s = state.settings;
  setText('chip-mode', s.mode === 'tun' ? t('allDevice') : t('proxyPort',{port:s.proxyPort}));
  const count = s.apps.length;
  setText('chip-split',
    s.split === 'all' || !count ? t('allApps')
      : s.split === 'only' ? t('onlyApps',{n:count})
        : t('exceptApps',{n:count}));

  const currentKey = requestKey();
  const dirty = live.tunnel === 'connected' && !live.busy && currentKey !== null
    && live.appliedKey !== null && live.appliedKey !== currentKey;
  setProperty($('apply'), 'hidden', !dirty);
}

$('power').addEventListener('click', () => {
  if (live.busy) return;
  if (live.tunnel === 'connected') disconnect(); else connect();
});
$('apply').addEventListener('click', connect);
$('server-card').addEventListener('click', () => showPage('servers'));
$('chip-mode').addEventListener('click', () => showPage('rules'));
$('chip-split').addEventListener('click', () => showPage('rules'));

// ---------- серверы ----------

function formatBytes(bytes) {
  const gb = bytes / 1024 ** 3;
  return `${new Intl.NumberFormat(FlyxI18n.locale(),{maximumFractionDigits:gb>=100?0:1,minimumFractionDigits:gb>=100?0:1}).format(gb)} ${t('gb')}`;
}

function profileInfo(profile) {
  const parts = [FlyxI18n.countServers(profile.nodes.length)];
  if (profile.used != null) parts.push(profile.total ? t('usage',{used:formatBytes(profile.used),total:formatBytes(profile.total)}) : formatBytes(profile.used));
  if (profile.expire) parts.push(t('until',{date:new Date(profile.expire * 1000).toLocaleDateString(FlyxI18n.locale())}));
  return parts.join(' · ');
}

function plural(n, one, few, many) {
  const m10 = n % 10, m100 = n % 100;
  if (m10 === 1 && m100 !== 11) return one;
  if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return few;
  return many;
}

const pingKey = (view) => `${view.server}:${view.port}`;

function paintPing(badge, key) {
  badge.className = 'node-ping';
  if (live.pinging.has(key)) { badge.textContent = '···'; badge.classList.add('pending'); return; }
  if (!live.pings.has(key)) { badge.textContent = ''; return; }
  const ms = live.pings.get(key);
  if (ms == null) { badge.textContent = '—'; return; }
  badge.textContent = `${ms} ${t('ms')}`;
  badge.classList.add(ms < 120 ? 'fast' : ms < 300 ? 'mid' : 'slow');
}

function pingBadge(view) {
  const key = pingKey(view);
  const badge = el('span', { class: 'node-ping', 'data-ping': key });
  paintPing(badge, key);
  return badge;
}

// Обновляет значения на месте: список не перерисовывается, флаги не мигают.
function repaintPing(key) {
  document.querySelectorAll('.node-ping').forEach((badge) => { if (badge.dataset.ping === key) paintPing(badge, key); });
}

const PING_PARALLEL = 8;

async function pingAll(profile) {
  if (live.pingingProfiles.has(profile.id) || !profile.nodes.length) return;
  // под активным туннелем замер шёл бы через VPN и ничего не значил
  if (live.tunnel === 'connected' && state.settings.mode === 'tun') { showAddMessage(t('pingTun'), 'bad'); return; }
  const targets = new Map(profile.nodes.map((view) => [pingKey(view), view]));
  live.pingingProfiles.add(profile.id);
  for (const key of targets.keys()) live.pinging.add(key);
  showAddMessage('');
  renderServers();

  const queue = [...targets];
  let answered = 0;
  const worker = async () => {
    for (let item; (item = queue.shift());) {
      const [key, view] = item;
      let ms = null;
      try { ms = await invoke('ping', { server: view.server, port: view.port }); } catch (e) { /* нет ответа */ }
      if (ms != null) answered += 1;
      live.pings.set(key, ms);
      live.pinging.delete(key);
      repaintPing(key);
    }
  };
  await Promise.all(Array.from({ length: Math.min(PING_PARALLEL, queue.length) }, worker));

  live.pingingProfiles.delete(profile.id);
  showAddMessage(t('pingDone', { ok: answered, total: targets.size }), answered ? 'ok' : 'bad');
  if (!$('page-servers').hidden) renderServers();
}

function renderServers() {
  const root = $('profiles');
  root.replaceChildren();
  if (!state.profiles.length) {
    root.append(el('div', { class: 'empty', text: t('emptyServers') }));
    return;
  }
  for (const profile of state.profiles) {
    const refresh = profile.source && el('button', { class: 'icon-button', title: t('refresh'), 'aria-label': t('refresh'), onclick: (e) => refreshProfile(profile, e.currentTarget) },
      icon(['M13 8a5 5 0 1 1-1.5-3.6', 'M13 3v2.5h-2.5']));
    const pinging = live.pingingProfiles.has(profile.id);
    const ping = profile.nodes.length > 0 && el('button', { class: `icon-button${pinging ? ' busy' : ''}`, title: t('pingAll'), 'aria-label': t('pingAll'), onclick: () => pingAll(profile) },
      el('span', { class: 'button-sticker', 'data-sticker': pinging ? 'loading' : 'ping', 'aria-hidden': 'true' }));
    if (ping) ping.disabled = pinging;
    const remove = el('button', { class: 'icon-button', title: t('delete'), 'aria-label': t('deleteSubscription'), onclick: () => removeProfile(profile) }, el('span', { class: 'button-sticker', 'data-sticker': 'remove', 'aria-hidden': 'true' }));

    const list = el('div', { class: 'glass nodes' });
    profile.nodes.forEach((view, index) => {
      const isSelected = state.selected && state.selected.profile === profile.id && state.selected.index === index;
      list.append(el('button', { class: `node${isSelected ? ' selected' : ''}`, onclick: () => selectNode(profile.id, index) },
        FlyxCountries.element(view.name),
        el('span', { class: 'card-text' },
          el('span', { class: 'card-title', text: FlyxCountries.label(view.name) }),
          el('span', { class: 'card-sub', text: `${describeNode(view)}${view.via_xray ? ` · ${t('viaXray')}` : ''}` })),
        pingBadge(view)));
    });

    if (!profile.nodes.length) list.append(el('div', { class:'empty', text:t('refreshEmpty') }));

    const quota = profile.total && profile.used != null
      ? el('div', { class: 'quota' }, el('i', { style: `width:${Math.min(100, (profile.used / profile.total) * 100).toFixed(1)}%` }))
      : null;

    root.append(el('div', { class: 'profile' },
      el('div', { class: 'profile-head' },
        el('div', { class: 'profile-title' },
          el('div', { class: 'profile-name', text: profile.id===MANUAL_ID?t('myKeys'):profile.name }),
          el('div', { class: 'profile-info', text: profileInfo(profile) })),
        ping, refresh, remove),
      quota, list));
  }
}

async function selectNode(profile, index) {
  if (live.busy || (state.selected?.profile === profile && state.selected.index === index)) return;
  const reconnect = live.tunnel === 'connected';
  state.selected = { profile, index };
  save();
  renderServers();
  renderHome();
  if (reconnect) await connect();
}

function removeProfile(profile) {
  state.profiles = state.profiles.filter((p) => p !== profile);
  if (state.selected && state.selected.profile === profile.id) state.selected = null;
  save();
  renderServers();
  renderHome();
}

function hostOf(url) {
  try { return new URL(url).hostname; } catch (e) { return t('servers'); }
}

function mergeImported(imported) {
  let profile;
  if (imported.source) {
    profile = state.profiles.find((p) => p.source === imported.source);
    if (!profile) {
      profile = { id: `sub-${Date.now()}`, source: imported.source, nodes: [] };
      state.profiles.push(profile);
    }
    // выбранный сервер ищем после обновления по имени: порядок в подписке мог поменяться
    const previous = state.selected && state.selected.profile === profile.id ? profile.nodes[state.selected.index] : null;
    profile.name = imported.title || profile.name || hostOf(imported.source);
    profile.nodes = imported.nodes;
    profile.used = imported.used; profile.total = imported.total; profile.expire = imported.expire;
    profile.updated = Date.now();
    if (previous) {
      const index = profile.nodes.findIndex((n) => n.name === previous.name);
      state.selected = index >= 0 ? { profile: profile.id, index } : null;
    }
  } else {
    profile = state.profiles.find((p) => p.id === MANUAL_ID);
    if (!profile) {
      profile = { id: MANUAL_ID, name: 'Мои ключи', source: null, nodes: [] };
      state.profiles.unshift(profile);
    }
    for (const view of imported.nodes) {
      const same = profile.nodes.findIndex((n) => n.server === view.server && n.port === view.port && n.name === view.name);
      if (same >= 0) profile.nodes[same] = view; else profile.nodes.push(view);
    }
  }
  if (!state.selected && profile.nodes.length) state.selected = { profile: profile.id, index: 0 };
  save();
  return profile;
}

function importMessage(imported) {
  const added=t('added',{count:FlyxI18n.countServers(imported.nodes.length)});
  return imported.skipped.length ? added+' '+t('skipped',{n:imported.skipped.length,reason:FlyxI18n.error(imported.skipped[0].reason)}) : added;
}

function showAddMessage(text, kind) {
  const box = $('add-message');
  box.hidden = !text;
  box.textContent = text || '';
  box.className = `hint ${kind || ''}`;
}

$('add-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const input = $('add-input');
  const value = input.value.trim();
  if (!value) return;
  $('add-button').disabled = true;
  FlyxStickers.set($('add-button').querySelector('[data-sticker]'), 'loading');
  showAddMessage(t('loading'));
  try {
    const imported = await invoke('import_source', { input: value });
    const profile = mergeImported(imported);
    input.value = '';
    showAddMessage(importMessage(imported), 'ok');
    renderServers();
    renderHome();
    measurePings(profile);
  } catch (e) {
    showAddMessage(FlyxI18n.error(e), 'bad');
  }
  $('add-button').disabled = false;
  FlyxStickers.set($('add-button').querySelector('[data-sticker]'), 'add');
});

async function refreshProfile(profile, button) {
  button.replaceChildren(el('span', { class: 'button-sticker', 'data-sticker': 'loading', 'aria-hidden': 'true' }));
  button.disabled = true;
  try {
    const imported = await invoke('import_source', { input: profile.source });
    mergeImported(imported);
    showAddMessage(t('updated',{name:profile.name}), 'ok');
  } catch (e) {
    showAddMessage(FlyxI18n.error(e), 'bad');
  }
  renderServers();
  renderHome();
  measurePings(profile);
}

async function measurePings(profile) {
  // под активным туннелем замер шёл бы через VPN и ничего не значил
  if (live.tunnel === 'connected' && state.settings.mode === 'tun') return;
  await Promise.all(profile.nodes.map(async (view) => {
    const key = `${view.server}:${view.port}`;
    try { live.pings.set(key, await invoke('ping', { server: view.server, port: view.port })); }
    catch (e) { live.pings.set(key, null); }
  }));
  if (!$('page-servers').hidden) renderServers();
}

// ---------- правила ----------

function bindSegment(id, get, set) {
  const root = $(id);
  const paint = () => root.querySelectorAll('button').forEach((b) => b.classList.toggle('active', b.dataset.value === get()));
  root.addEventListener('click', (event) => {
    const button = event.target.closest('button');
    if (!button) return;
    set(button.dataset.value);
    save();
    renderRules();
    renderHome();
  });
  return paint;
}

const paintMode = bindSegment('seg-mode', () => state.settings.mode, (v) => { state.settings.mode = v; });
const paintSplit = bindSegment('seg-split', () => state.settings.split, (v) => { state.settings.split = v; });

function renderTags(rootId, items, onRemove) {
  const root = $(rootId);
  root.replaceChildren(...items.map((item) =>
    el('span', { class: 'tag' }, el('span', { text: item }),
      el('button', { 'aria-label': t('removeItem',{item}), text: '✕', onclick: () => onRemove(item) }))));
}

function renderRuCatalog() {
  const categories=Object.entries(FlyxRuServices.groups).map(([key,entries])=>{
    const services=entries.map(([name,...domains])=>el('div',{class:'ru-service'},
      el('span',{text:name,dir:'auto'}),el('small',{text:domains.join(', '),dir:'ltr'})));
    return el('div',{class:'ru-category'},el('strong',{text:t(key)}),...services);
  });
  $('ru-catalog-content').replaceChildren(...categories);
}
function renderRules() {
  const s = state.settings;
  paintMode();
  paintSplit();
  $('mode-hint').textContent = s.mode === 'tun'
    ? t('tunHint')
    : t('proxyHint',{port:s.proxyPort});
  $('split-hint').textContent = {
    all: t('splitAll'),
    except: t('splitExcept'),
    only: t('splitOnly'),
  }[s.split];
  $('apps-box').hidden = s.split === 'all';
  $('ru-toggle').checked = s.ruDirect;
  const removeFrom = (list) => (item) => { list.splice(list.indexOf(item), 1); save(); renderRules(); renderHome(); };
  renderTags('apps', s.apps, removeFrom(s.apps));
  renderTags('domains', s.domains, removeFrom(s.domains));
}

function bindAdder(formId, inputId, list, normalize) {
  $(formId).addEventListener('submit', (event) => {
    event.preventDefault();
    const value = normalize($(inputId).value.trim());
    if (value && !list().includes(value)) list().push(value);
    $(inputId).value = '';
    save();
    renderRules();
    renderHome();
  });
}

bindAdder('app-form', 'app-input', () => state.settings.apps, (v) => {
  if (!v) return '';
  const name = v.split(/[\\/]/).pop();
  if (platformOS !== 'Windows') return name;
  const windowsName = name.toLowerCase();
  return windowsName.endsWith('.exe') ? windowsName : `${windowsName}.exe`;
});
bindAdder('domain-form', 'domain-input', () => state.settings.domains, (v) =>
  v.toLowerCase().replace(/^[a-z]+:\/\//, '').replace(/\/.*$/, '').replace(/^\*?\./, ''));

$('ru-toggle').addEventListener('change', (event) => {
  state.settings.ruDirect = event.target.checked;
  save();
  renderHome();
});

// ---------- вкладки, окно, журнал ----------

const PAGES = ['home', 'servers', 'rules', 'settings'];

// Decorative mascots only animate on the visible portion of the current tab.
const settingsMotion = matchMedia('(prefers-reduced-motion: reduce)');
const pageMascots = [...document.querySelectorAll('[data-page-mascot]')];
const visibleMascots = new Set();
function syncPageMascots() {
  for (const image of pageMascots) {
    const animate = visibleMascots.has(image) && !window.FlyxUnfocused && !image.closest('.page').hidden
      && !document.hidden && !window.FlyxBackground && !settingsMotion.matches;
    const src = animate ? image.dataset.animatedSrc : 'assets/settings-mascot-still.png';
    if (image.getAttribute('src') !== src) image.src = src;
  }
}
const pageMascotObserver = new IntersectionObserver(entries => {
  for (const entry of entries) {
    if (entry.isIntersecting) visibleMascots.add(entry.target);
    else visibleMascots.delete(entry.target);
  }
  syncPageMascots();
});
pageMascots.forEach(image => pageMascotObserver.observe(image));
settingsMotion.addEventListener('change', syncPageMascots);
document.addEventListener('visibilitychange', syncPageMascots);
window.addEventListener('flyx-background', syncPageMascots);

function applyTheme(theme) {
  state.preferences.theme = ['light','dark','flyx'].includes(theme) ? theme : 'flyx';
  document.documentElement.dataset.theme = state.preferences.theme;
  document.querySelectorAll('[data-theme-choice]').forEach(button => {
    const active = button.dataset.themeChoice === state.preferences.theme;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  });
}
for (const button of document.querySelectorAll('[data-theme-choice]')) {
  button.addEventListener('click', () => { applyTheme(button.dataset.themeChoice); save(); });
}

function showPage(name) {
  for (const page of PAGES) $(`page-${page}`).hidden = page !== name;
  toggleLanguageMenu(false);
  syncPageMascots();
  document.querySelectorAll('.tab').forEach((tab) => tab.classList.toggle('active', tab.dataset.page === name));
  $('tab-thumb').style.transform = `translateX(${PAGES.indexOf(name) * 100 * (document.documentElement.dir==='rtl'?-1:1)}%)`;
  if (name === 'servers') { renderServers(); state.profiles.forEach(measurePings); }
  if (name === 'rules') renderRules();
}

document.querySelectorAll('.tab').forEach((tab) => tab.addEventListener('click', () => showPage(tab.dataset.page)));

if (TAURI) {
  const appWindow = TAURI.window.getCurrentWindow();
  $('win-min').addEventListener('click', () => appWindow.minimize());
  $('win-close').addEventListener('click', () => appWindow.close());
  const backgroundMode = (hidden) => {
    window.FlyxBackground = Boolean(hidden);
    window.dispatchEvent(new Event('flyx-background'));
    renderHome();
    if (!hidden) refreshStatus();
  };
  TAURI.event.listen('app-background', ({ payload }) => backgroundMode(payload));
  Promise.all([appWindow.isVisible(), appWindow.isMinimized()]).then(([visible, minimized]) => backgroundMode(!visible || minimized));
  TAURI.event.listen('tray-error', ({ payload }) => { live.error = String(payload); renderHome(); });
}

// ---------- журнал ----------

const logView = { filter: 'all', raw: [], rows: [], timer: 0, shown: '' };

// Строки ядер приводятся к одному виду: время, уровень, ядро, сообщение.
//   [sing-box] +0300 2026-11-16 00:33:56 INFO [2883497829 0ms] router: …
//   [xray] 2026/11/16 00:33:55.738849 [Info] [3335378350] proxy/vless/outbound: …
function parseLogLine(raw) {
  const tagged = raw.match(/^\[([\w-]+)\]\s+(.*)$/);
  const core = tagged ? tagged[1] : '';
  let text = tagged ? tagged[2] : raw, time = '', level = '';
  let m = text.match(/^[+-]\d{4}\s+\d{4}-\d{2}-\d{2}\s+(\d{2}:\d{2}:\d{2})\s+([A-Z]+)\s+(?:\[\d+ [\d.]+m?s\]\s+)?(.*)$/);
  if (m) [, time, level, text] = m;
  else if ((m = text.match(/^\d{4}\/\d{2}\/\d{2}\s+(\d{2}:\d{2}:\d{2})(?:\.\d+)?\s+\[(\w+)\]\s+(?:\[\d+\]\s+)?(.*)$/))) { time = m[1]; level = m[2].toUpperCase(); text = m[3]; }
  const kind = /ERROR|FATAL|PANIC/.test(level) ? 'error' : /WARN/.test(level) ? 'warn' : 'info';
  return { raw, core, time, kind, text };
}

function visibleLogRows() {
  return logView.filter === 'all' ? logView.rows : logView.rows.filter((row) => row.kind !== 'info');
}

function renderLogs(message) {
  const list = $('log-lines');
  const rows = visibleLogRows();
  // не перерисовываем то же самое: иначе сбрасывается выделение текста
  const signature = message || `${logView.filter}:${rows.length}:${rows.length ? rows[rows.length - 1].raw : ''}`;
  if (signature === logView.shown) return;
  logView.shown = signature;
  const atBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 48;
  if (message || !rows.length) {
    list.replaceChildren(el('div', { class: 'log-empty', text: message || t('noLogs') }));
    return;
  }
  list.replaceChildren(...rows.map((row) => el('div', { class: `log-row ${row.kind}` },
    el('span', { class: 'log-meta' }, el('i', { class: 'log-dot' }), el('span', { text: row.time || '··:··:··' })),
    el('span', { class: 'log-text', dir: 'ltr' }, row.core ? el('b', { class: 'log-core', text: row.core }) : null, document.createTextNode(row.text)))));
  if (atBottom) list.scrollTop = list.scrollHeight;
}

async function loadLogs() {
  try {
    const response = await invoke('svc_logs', { lines: 300 });
    logView.raw = response.logs || [];
    logView.rows = logView.raw.map(parseLogLine);
    renderLogs();
  } catch (e) {
    logView.raw = []; logView.rows = [];
    renderLogs(FlyxI18n.error(e));
  }
}

async function openLogs() {
  $('sheet').hidden = false;
  logView.shown = '';
  renderLogs(t('loading'));
  await loadLogs();
  $('log-lines').scrollTop = $('log-lines').scrollHeight;
  clearInterval(logView.timer);
  // пока журнал открыт, он дополняется сам
  logView.timer = setInterval(() => { if (!document.hidden && !window.FlyxBackground) loadLogs(); }, 2000);
}
function closeLogs() {
  $('sheet').hidden = true;
  clearInterval(logView.timer);
}

$('log-filter').addEventListener('click', (event) => {
  const button = event.target.closest('button');
  if (!button) return;
  logView.filter = button.dataset.value;
  $('log-filter').querySelectorAll('button').forEach((b) => b.classList.toggle('active', b === button));
  renderLogs();
  $('log-lines').scrollTop = $('log-lines').scrollHeight;
});
$('log-copy').addEventListener('click', async () => {
  const text = visibleLogRows().map((row) => row.raw).join('\n');
  try { await navigator.clipboard.writeText(text); } catch (e) { return; }
  $('log-copy').textContent = t('logCopied');
  setTimeout(() => { $('log-copy').textContent = t('logCopy'); }, 1400);
});
$('notice-logs').addEventListener('click', openLogs);
$('sheet-close').addEventListener('click', closeLogs);
$('sheet-shade').addEventListener('click', closeLogs);
document.addEventListener('keydown', (event) => { if (event.key === 'Escape' && !$('sheet').hidden) closeLogs(); });

// ---------- запуск ----------

async function start() {
  try { const os=await invoke('platform_info'); platformOS=['Windows','Linux','Android'].includes(os)?os:'Windows'; } catch {}
  FlyxI18n.setPlatform(platformOS);
  $('app-input').placeholder = platformOS === 'Android' ? 'org.telegram.messenger' : platformOS === 'Linux' ? 'firefox' : 'chrome.exe';
  if(platformOS==='Android') {
    document.documentElement.classList.add('android');
    $('seg-mode').hidden=true;
    $('start-hidden').closest('label').hidden=true;
  }
  try {
    const saved = await invoke('load_state');
    if (saved && typeof saved === 'object') {
      if (Array.isArray(saved.profiles)) state.profiles = saved.profiles.map(profile => ({ ...profile, nodes:(profile.nodes || []).filter(view => !providerNotice(view.name)) }));
      if (saved.selected) state.selected = saved.selected;
      Object.assign(state.settings, saved.settings || {});
      Object.assign(state.preferences, saved.preferences || {});
    }
  } catch (e) { /* первый запуск */ }
  applyTheme(state.preferences.theme);
  if(platformOS==='Android')state.settings.mode='tun';
  applyLanguage(state.preferences.language);
  renderPreferences();
  renderHome();
  await refreshStatus();
  // туннель мог остаться с прошлого запуска: считаем его настройки текущими
  if (live.tunnel === 'connected') live.appliedKey = requestKey();
  renderHome();
  if (state.preferences.autoConnect === true && live.service === 'ok' && live.tunnel !== 'connected' && selectedNode()) await connect();
  setInterval(() => { if (!document.hidden && !window.FlyxBackground) refreshStatus(); }, 2000);
  setInterval(() => { if (!window.FlyxBackground && live.tunnel === 'connected' && !live.busy && live.since) setText('status-sub', formatElapsed(live.since)); }, 1000);
}

function renderPreferences() {
  $('auto-connect').checked = state.preferences.autoConnect===true;
  $('start-hidden').checked = state.preferences.startHidden===true;
}
function applyLanguage(language) {
  state.preferences.language=FlyxI18n.set(language);
  $('language-select').value=state.preferences.language;
  renderLanguagePicker();
  renderHome(); renderRules(); renderRuCatalog();
  if (!$('page-servers').hidden) renderServers();
  const page=document.querySelector('.tab.active')?.dataset.page || 'home';
  $('tab-thumb').style.transform=`translateX(${PAGES.indexOf(page)*100*(document.documentElement.dir==='rtl'?-1:1)}%)`;
  if (typeof ad!=='undefined' && ad.enabled) $('ad-label').textContent=t('ad');
  invoke('set_ui_language',{language:state.preferences.language}).catch(()=>{});
}
for (const [code,name] of FlyxI18n.languages) $('language-select').append(el('option',{value:code,text:name+' · '+code.toUpperCase(),dir:'auto'}));
$('language-select').addEventListener('change',event=>{applyLanguage(event.target.value);save();});

// Свой список языков вместо системного: тот выпадал белым прямоугольником и ломал строку с фарси.
function toggleLanguageMenu(open) {
  $('language-menu').hidden = !open;
  $('language-button').setAttribute('aria-expanded', String(open));
  // нижний ряд не должен оставаться под панелью вкладок
  if (open) $('language-menu').scrollIntoView({ block: 'nearest', behavior: 'smooth' });
}
function renderLanguagePicker() {
  const current = state.preferences.language;
  const [code, name] = FlyxI18n.languages.find(([value]) => value === current) || FlyxI18n.languages[0];
  setText('language-name', name);
  setText('language-code', code.toUpperCase());
  $('language-menu').replaceChildren(...FlyxI18n.languages.map(([value, label]) =>
    el('button', { type: 'button', class: 'language-option', role: 'option', lang: value, 'aria-selected': String(value === current),
      onclick: () => { toggleLanguageMenu(false); if (value !== state.preferences.language) { applyLanguage(value); save(); } } },
      el('span', { text: label, dir: 'auto' }), el('small', { text: value.toUpperCase() }))));
}
$('language-button').addEventListener('click', () => toggleLanguageMenu($('language-menu').hidden));
document.addEventListener('keydown', (event) => { if (event.key === 'Escape') toggleLanguageMenu(false); });
for (const [id,key] of [['auto-connect','autoConnect'],['start-hidden','startHidden']]) {
  $(id).addEventListener('change',event=>{state.preferences[key]=event.target.checked;save();});
}
$('settings-logs').addEventListener('click',openLogs);
start();

// ---------- заглушка для просмотра в браузере, без приложения ----------

function mockInvoke(command, args) {
  const mock = mockInvoke.state || (mockInvoke.state = { tunnel: window.__FLYX_PREVIEW_TUNNEL__ || 'idle', since: null, server: null });
  const reply = () => ({ ok: true, state: mock.tunnel, server: mock.server, since: mock.since, error: null, logs: [], version: 'preview' });
  const wait = (ms, value) => new Promise((resolve) => setTimeout(() => resolve(value), ms));
  const sample = (name, server, proto, transport, security, via_xray) => ({ name, server, port: 443, proto, transport, security, via_xray, node: { name, server, port: 443 } });
  switch (command) {
    case 'platform_info': return Promise.resolve(window.__FLYX_PREVIEW_PLATFORM__ || 'Windows');
    case 'load_state': return wait(10, window.__FLYX_PREVIEW_STATE__ || null);
    case 'save_state': mock.saved=structuredClone(args.state); return wait(10);
    case 'set_ui_language': return wait(10);
    case 'svc_status': return wait(30, reply());
    case 'svc_connect': mock.connectCalls=(mock.connectCalls||0)+1; mock.tunnel = 'connected'; mock.since = Math.floor(Date.now() / 1000); mock.server = args.request.node.name; return wait(1400, reply());
    case 'svc_disconnect': mock.tunnel = 'idle'; mock.since = null; mock.server = null; return wait(500, reply());
    case 'svc_logs': return wait(100, { ...reply(), logs: ['[sing-box] INFO sing-box started (0.15s)', '[sing-box] INFO inbound/tun[tun-in]: started'] });
    case 'ping': return wait(200 + Math.random() * 400, Math.random() < 0.15 ? null : Math.round(30 + Math.random() * 320));
    case 'import_source': return wait(600, {
      source: args.input.startsWith('http') ? args.input : null, title: args.input.startsWith('http') ? 'Demo VPN' : null,
      used: 12.4 * 1024 ** 3, total: 100 * 1024 ** 3, expire: Math.floor(Date.now() / 1000) + 86400 * 21, skipped: [],
      nodes: [sample('Амстердам', 'nl.example.com', 'vless', 'tcp', 'reality', false), sample('Франкфурт', 'de.example.com', 'trojan', 'ws', 'tls', false),
        sample('Хельсинки', 'fi.example.com', 'hysteria2', 'tcp', 'tls', false), sample('Стокгольм XHTTP', 'se.example.com', 'vless', 'xhttp', 'reality', true)],
    });
    default: return Promise.reject(`нет команды ${command}`);
  }
}

const ad = window.FLYX_AD || {};
if (ad.enabled && typeof ad.image === 'string' && /^assets\/[a-zA-Z0-9_./-]+$/.test(ad.image) && !ad.image.includes('..')) {
  $('ad-slot').hidden = false;
  $('ad-image').src = ad.image;
  $('ad-image').alt = ad.alt || t('ad');
  $('ad-label').textContent = t('ad');
  $('ad-advertiser').textContent = ad.advertiser || '';
  const clickable = typeof ad.url === 'string' && /^https:\/\//i.test(ad.url);
  $('ad-slot').dataset.clickable = String(clickable);
  $('ad-slot').setAttribute('aria-label', (ad.alt || t('ad')) + (clickable ? ' — '+t('adOpen') : ''));
  $('ad-slot').addEventListener('click', async () => {
    if (!clickable) return;
    if (TAURI) {
      try { await invoke('open_ad_url', { url: ad.url }); } catch (e) { $('notice').hidden = false; $('notice-text').textContent = FlyxI18n.error(e); }
    } else window.open(ad.url, '_blank', 'noopener,noreferrer');
  });
}

document.addEventListener('visibilitychange', () => { if (!document.hidden) refreshStatus(); });
