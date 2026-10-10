/* sniffnetWRT frontend — vanilla JS, no dependencies. */
'use strict';

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
const WS_PATH = '/ws';
const TOKEN = (new URLSearchParams(location.search)).get('token') || '';

const state = {
  last: null,
  series: [],            // ring buffer of {t, in, out} rates (bytes/s)
  sortKey: 'bytes',
  sortAsc: false,
  search: '',
  dirFilter: '',
  catFilter: '',
  theme: localStorage.getItem('sniffnetwrt.theme') || 'dark',
  expanded: null,
  lastNotify: 0,
  rateInSm: null,   // EMA-smoothed incoming rate (anti-jitter)
  rateOutSm: null,  // EMA-smoothed outgoing rate
  updateInterval: Number(localStorage.getItem('sniffnetwrt.updateInterval') || 1),
  pendingSnapshot: null,
  lastRenderedAt: 0,
  renderTimer: null,
  securitySearch: '',
  securityTarget: '',
  devicePrevious: new Map(),
  deviceRates: new Map(),
  knownDeviceIds: null,
  rateBaseline: null,
  rateSampleCount: 0,
  lastDeviceAlert: 0,
  lastSpikeAlert: 0,
};

// ---------------------------------------------------------------------------
// DOM refs
// ---------------------------------------------------------------------------
const $ = (id) => document.getElementById(id);
const els = {
  iface: $('iface'), status: $('status'), statusText: $('statusText'), dot: $('dot'), clock: $('clock'),
  hLoad: $('hLoad'), hLoadSub: $('hLoadSub'), hMem: $('hMem'), hMemSub: $('hMemSub'),
  hTemp: $('hTemp'), hUp: $('hUp'), hUpSub: $('hUpSub'),
  rateIn: $('rateIn'), rateInSub: $('rateInSub'), rateOut: $('rateOut'), rateOutSub: $('rateOutSub'),
  totalData: $('totalData'), totalIn: $('totalIn'), totalOut: $('totalOut'),
  connCount: $('connCount'), pktCount: $('pktCount'), pktSub: $('pktSub'),
  services: $('services'), chart: $('chart'),
  wanDev: $('wanDev'), wanIp: $('wanIp'), wanUp: $('wanUp'), wanRx: $('wanRx'), wanTx: $('wanTx'),
  search: $('search'), catFilter: $('catFilter'), dirFilter: $('dirFilter'),
  connSub: $('connSub'), connBody: $('connBody'),
  devSub: $('devSub'), devBody: $('devBody'),
  notifyEnabled: $('notifyEnabled'), notifyThreshold: $('notifyThreshold'),
  updateInterval: $('updateInterval'),
  securityNavCount: $('securityNavCount'), securityState: $('securityState'), securityStateText: $('securityStateText'),
  securityNotice: $('securityNotice'), secTotal: $('secTotal'), secSources: $('secSources'),
  secTopSource: $('secTopSource'), secTopSourceSub: $('secTopSourceSub'), secTopPort: $('secTopPort'), secTopPortSub: $('secTopPortSub'),
  securityCount: $('securityCount'), securityBody: $('securityBody'), securitySearch: $('securitySearch'),
  securityTarget: $('securityTarget'), clearSecurity: $('clearSecurity'),
  exportConnections: $('exportConnections'), exportSecurity: $('exportSecurity'),
  alertNewDevice: $('alertNewDevice'), alertRateSpike: $('alertRateSpike'),
  securityRetention: $('securityRetention'), hideDomains: $('hideDomains'),
};

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------
function fmtBytes(n) {
  if (!Number.isFinite(n) || n < 0) n = 0;
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (n >= 1000 && i < units.length - 1) { n /= 1000; i++; }
  return `${i === 0 ? n : n.toFixed(1)} ${units[i]}`;
}
function fmtRate(bps) {
  if (!Number.isFinite(bps) || bps < 0) bps = 0;
  const units = ['bps', 'Kbps', 'Mbps', 'Gbps'];
  let i = 0;
  while (bps >= 1000 && i < units.length - 1) { bps /= 1000; i++; }
  return `${i === 0 ? bps.toFixed(0) : bps.toFixed(1)} ${units[i]}`;
}
function fmtUptime(secs) {
  secs = Math.max(0, Math.floor(secs || 0));
  const d = Math.floor(secs / 86400), h = Math.floor((secs % 86400) / 3600), m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}
function timeHms(sec) { return new Date(sec * 1000).toTimeString().slice(0, 8); }
function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
function ema(prev, next, alpha) {
  return prev == null ? next : prev * (1 - alpha) + next * alpha;
}

// ---------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------
setInterval(() => { els.clock.textContent = new Date().toLocaleTimeString(); }, 1000);

// ---------------------------------------------------------------------------
// Chart (dependency-free canvas area chart)
// ---------------------------------------------------------------------------
const chart = {
  maxPoints: 120,
  ctx: els.chart.getContext('2d'),
  yMax: 1024,
  w: 0, h: 280,
  resize() {
    const dpr = window.devicePixelRatio || 1;
    const { width, height } = els.chart.getBoundingClientRect();
    els.chart.width = Math.max(1, Math.floor(width * dpr));
    els.chart.height = Math.max(1, Math.floor(height * dpr));
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.w = width; this.h = height;
  },
  push(inRate, outRate) {
    state.series.push({ t: Date.now(), in: inRate, out: outRate });
    if (state.series.length > this.maxPoints) state.series.shift();
  },
  draw() {
    const { ctx, w, h } = this;
    if (!w) return;
    ctx.clearRect(0, 0, w, h);
    const padL = 54, padR = 10, padT = 10, padB = 22;
    const plotW = w - padL - padR, plotH = h - padT - padB;

    let peak = 0;
    for (const p of state.series) peak = Math.max(peak, p.in, p.out);
    this.yMax = Math.max(1024, peak * 1.15);

    ctx.strokeStyle = 'rgba(139,148,158,0.15)';
    ctx.fillStyle = '#8b949e';
    ctx.font = '10px ui-monospace, monospace';
    ctx.textAlign = 'right'; ctx.textBaseline = 'middle';
    const steps = 4;
    for (let i = 0; i <= steps; i++) {
      const y = padT + (plotH * i) / steps;
      ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(w - padR, y); ctx.stroke();
      ctx.fillText(fmtRate(this.yMax * (1 - i / steps)), padL - 6, y);
    }

    const x = (idx) => state.series.length <= 1 ? padL : padL + (plotW * idx) / (state.series.length - 1);
    const y = (v) => padT + plotH * (1 - Math.min(v, this.yMax) / this.yMax);

    ctx.textAlign = 'center'; ctx.textBaseline = 'top';
    const first = state.series[0]?.t, lastT = state.series[state.series.length - 1]?.t;
    const span = lastT - first;
    if (span > 0 && first) {
      for (let t = Math.ceil(first / 30000) * 30000; t <= lastT; t += 30000) {
        ctx.fillText(new Date(t).toTimeString().slice(0, 5), padL + plotW * ((t - first) / span), h - padB + 6);
      }
    }

    const drawSeries = (key, color) => {
      if (state.series.length < 2) return;
      ctx.beginPath();
      state.series.forEach((p, i) => { const px = x(i), py = y(p[key]); i === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py); });
      ctx.strokeStyle = color; ctx.lineWidth = 2; ctx.lineJoin = 'round'; ctx.stroke();
      ctx.lineTo(x(state.series.length - 1), padT + plotH); ctx.lineTo(x(0), padT + plotH); ctx.closePath();
      const grad = ctx.createLinearGradient(0, padT, 0, padT + plotH);
      grad.addColorStop(0, hexToRgba(color, 0.28)); grad.addColorStop(1, hexToRgba(color, 0));
      ctx.fillStyle = grad; ctx.fill();
    };
    drawSeries('in', getComputedStyle(document.body).getPropertyValue('--in').trim() || '#2dd4bf');
    drawSeries('out', getComputedStyle(document.body).getPropertyValue('--out').trim() || '#f59e0b');
  },
};
function hexToRgba(hex, a) {
  const r = parseInt(hex.slice(1, 3), 16), g = parseInt(hex.slice(3, 5), 16), b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${a})`;
}
window.addEventListener('resize', () => { chart.resize(); chart.draw(); });

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------
function render(snap) {
  state.lastRenderedAt = Date.now();
  state.last = snap;
  els.iface.textContent = snap.interface || 'wan';
  setStatus(true);

  const t = snap.totals || {};
  const smoothing = 1 - Math.pow(1 - 0.35, state.updateInterval);
  state.rateInSm = ema(state.rateInSm, t.incoming_rate || 0, smoothing);
  state.rateOutSm = ema(state.rateOutSm, t.outgoing_rate || 0, smoothing);
  els.rateIn.textContent = fmtRate(state.rateInSm);
  els.rateInSub.textContent = `${fmtBytes(state.rateInSm)}/s`;
  els.rateOut.textContent = fmtRate(state.rateOutSm);
  els.rateOutSub.textContent = `${fmtBytes(state.rateOutSm)}/s`;
  els.totalData.textContent = fmtBytes((t.incoming_bytes || 0) + (t.outgoing_bytes || 0));
  els.totalIn.textContent = fmtBytes(t.incoming_bytes || 0);
  els.totalOut.textContent = fmtBytes(t.outgoing_bytes || 0);
  els.pktCount.textContent = (t.incoming_packets || 0) + (t.outgoing_packets || 0);

  chart.push(t.incoming_rate || 0, t.outgoing_rate || 0);
  chart.draw();

  renderHealth(snap.health);
  renderWan(snap.wan);
  renderServices(snap.services || []);
  renderDevices(snap.devices || []);
  buildCategoryFilter(snap.connections || []);
  renderTable();
  renderSecurity(snap);

}

function scheduleSnapshot(snap) {
  checkNotifications(snap);
  sampleDeviceRates(snap);
  state.pendingSnapshot = snap;
  if (state.renderTimer) return;
  const wait = Math.max(0, state.updateInterval * 1000 - (Date.now() - state.lastRenderedAt));
  state.renderTimer = setTimeout(() => {
    state.renderTimer = null;
    const latest = state.pendingSnapshot;
    state.pendingSnapshot = null;
    if (latest) render(latest);
  }, wait);
}

function renderHealth(h) {
  if (!h) return;
  els.hLoad.textContent = h.load1.toFixed(2);
  els.hLoadSub.textContent = `${h.load5.toFixed(2)} / ${h.load15.toFixed(2)}`;
  els.hMem.textContent = fmtBytes(h.mem_used_kb * 1000);
  els.hMemSub.textContent = `of ${fmtBytes(h.mem_total_kb * 1000)}`;
  els.hTemp.textContent = h.temp_c != null ? `${h.temp_c.toFixed(1)}°C` : '—';
  els.hUp.textContent = fmtUptime(h.uptime_secs);
}

function renderWan(w) {
  if (!w) return;
  els.wanDev.textContent = w.device || '';
  els.wanIp.textContent = w.ip || '—';
  els.wanUp.textContent = fmtUptime(w.uptime_secs);
  els.wanRx.textContent = fmtBytes(w.rx_bytes);
  els.wanTx.textContent = fmtBytes(w.tx_bytes);
}

function renderServices(services) {
  if (!services.length) { els.services.innerHTML = '<li class="empty">waiting for traffic…</li>'; return; }
  const max = Math.max(...services.map((s) => s.bytes), 1);
  els.services.innerHTML = services.slice(0, 12).map((s) => `
    <li>
      <span class="name" title="${escapeHtml(s.name)}">${escapeHtml(s.name)}</span>
      <span class="bar-track"><span class="bar" style="width:${(s.bytes / max) * 100}%"></span></span>
      <span class="bytes">${fmtBytes(s.bytes)}</span>
    </li>`).join('');
}

function renderDevices(devices) {
  const online = devices.filter((d) => d.online).length;
  els.devSub.textContent = `${online} online`;
  if (!devices.length) { els.devBody.innerHTML = '<tr><td colspan="6" class="empty">no devices discovered</td></tr>'; return; }
  els.devBody.innerHTML = devices
    .slice().sort((a, b) => Number(b.online) - Number(a.online) || (b.incoming_bytes + b.outgoing_bytes) - (a.incoming_bytes + a.outgoing_bytes))
    .map((d) => `
      <tr>
        <td>${escapeHtml(d.hostname || '—')}</td>
        <td>${escapeHtml(d.mac)}</td>
        <td>${escapeHtml(d.ip)}</td>
        <td class="num in"><span>${fmtBytes(d.incoming_bytes || 0)}</span><small class="cell-rate">${fmtBytes(state.deviceRates.get(d.mac)?.down || 0)}/s</small></td>
        <td class="num out"><span>${fmtBytes(d.outgoing_bytes || 0)}</span><small class="cell-rate">${fmtBytes(state.deviceRates.get(d.mac)?.up || 0)}/s</small></td>
        <td><span class="status-badge ${d.online ? 'online' : ''}"><span class="dot"></span>${d.online ? 'online' : 'offline'}</span></td>
      </tr>`).join('');
}

function sampleDeviceRates(snap) {
  const now = Number(snap.timestamp) || Date.now() / 1000;
  for (const device of snap.devices || []) {
    const id = device.mac || device.ip;
    const down = Number(device.incoming_bytes) || 0;
    const up = Number(device.outgoing_bytes) || 0;
    const previous = state.devicePrevious.get(id);
    if (previous && now > previous.time) {
      const elapsed = now - previous.time;
      state.deviceRates.set(id, {
        down: Math.max(0, down - previous.down) / elapsed,
        up: Math.max(0, up - previous.up) / elapsed,
      });
    }
    state.devicePrevious.set(id, { down, up, time: now });
  }
}

function buildCategoryFilter(conns) {
  const cats = [...new Set(conns.map((c) => c.category).filter(Boolean))].sort();
  const cur = els.catFilter.value;
  els.catFilter.innerHTML = '<option value="">All categories</option>' + cats.map((c) => `<option value="${escapeHtml(c)}">${escapeHtml(c)}</option>`).join('');
  els.catFilter.value = cur;
}

// ---------------------------------------------------------------------------
// Connections table + filters
// ---------------------------------------------------------------------------
function filteredConnections() {
  const conns = (state.last?.connections || []);
  const q = state.search.toLowerCase();
  return conns.filter((c) => {
    if (state.dirFilter && c.direction !== state.dirFilter) return false;
    if (state.catFilter && c.category !== state.catFilter) return false;
    if (q) {
      const domain = els.hideDomains.checked ? '' : (c.domain || '');
      const hay = `${c.src_ip} ${c.dst_ip} ${c.service} ${c.category} ${c.protocol} ${domain} ${c.asn || ''} ${c.country || ''}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

function renderTable() {
  const conns = filteredConnections();
  els.connCount.textContent = (state.last?.connections || []).length;
  els.connSub.textContent = `${conns.length} shown`;
  if (!conns.length) {
    els.connBody.innerHTML = '<tr><td colspan="9" class="empty">no traffic yet</td></tr>';
    return;
  }
  const sorted = [...conns].sort(cmpConn);
  els.connBody.innerHTML = sorted.map((c) => rowHtml(c) + detailHtml(c)).join('');
}

function rowHtml(c) {
  return `<tr data-id="${escapeHtml(c.id)}">
    <td>${fmtEndpoint(c.src_ip, c.src_port)}</td>
    <td>${fmtEndpoint(c.dst_ip, c.dst_port)}${hostLine(c)}</td>
    <td class="geo-cell">${countryBadge(c.country)}</td>
    <td>${escapeHtml(c.protocol)}</td>
    <td><span class="badge ${escapeHtml(c.service)}">${escapeHtml(c.service)}</span></td>
    <td class="dir ${c.direction === 'incoming' ? 'in' : 'out'}">${c.direction === 'incoming' ? '▼' : '▲'}</td>
    <td class="num">${fmtBytes(c.bytes)}</td>
    <td class="num">${c.packets}</td>
    <td class="num">${timeHms(c.last_seen)}</td>
  </tr>`;
}

function detailHtml(c) {
  const rows = [];
  if (!els.hideDomains.checked && c.domain) rows.push(`<span><b>Domain</b> ${escapeHtml(c.domain)}</span>`);
  if (c.country) rows.push(`<span><b>Country</b> ${escapeHtml(c.country)}</span>`);
  if (c.asn) rows.push(`<span><b>ASN</b> ${escapeHtml(c.asn)}</span>`);
  rows.push(`<span><b>Category</b> ${escapeHtml(c.category || '—')}</span>`);
  rows.push(`<span><b>First seen</b> ${timeHms(c.first_seen)}</span>`);
  rows.push(`<span><b>In/Out</b> ${fmtBytes(c.incoming_bytes)} / ${fmtBytes(c.outgoing_bytes)}</span>`);
  return `<tr class="detail-row hidden" data-detail="${escapeHtml(c.id)}"><td colspan="9"><div class="detail-grid">${rows.join('')}</div></td></tr>`;
}

function fmtEndpoint(ip, port) { return port ? `${ip}:${port}` : ip; }
function hostLine(c) {
  if (els.hideDomains.checked) return '';
  const parts = [];
  if (c.domain && c.domain !== c.dst_ip && c.domain !== c.src_ip) parts.push(escapeHtml(c.domain));
  if (c.asn) parts.push(escapeHtml(c.asn));
  return parts.length ? `<div class="host">${parts.join(' · ')}</div>` : '';
}
function countryBadge(country) {
  if (!country || !/^[A-Z]{2}$/.test(country)) return '<span class="geo-unknown" title="GeoIP data unavailable">—</span>';
  const code = country.toLowerCase();
  return `<span class="country-badge" title="${escapeHtml(country)}"><img class="country-flag" src="/flags/${code}.svg" alt="" loading="lazy" decoding="async"><span>${escapeHtml(country)}</span></span>`;
}

// ---------------------------------------------------------------------------
// Firewall rejection event stream (RAM only)
// ---------------------------------------------------------------------------
function currentSecurityEvents(snap = state.last) {
  return snap?.security_events || [];
}
function renderSecurity(snap) {
  const enabled = Boolean(snap.security_logging);
  const cutoff = Date.now() / 1000 - Number(els.securityRetention.value || 3600);
  const events = currentSecurityEvents(snap).filter((event) => Number(event.timestamp) >= cutoff);
  els.securityState.classList.toggle('enabled', enabled);
  els.securityState.classList.toggle('disabled', !enabled);
  els.securityStateText.textContent = enabled ? 'Logging configured · RAM only' : 'Logging off';
  els.securityNotice.classList.toggle('hidden', enabled);
  if (!enabled) {
    els.securityNotice.innerHTML = '<strong>Firewall event capture is off.</strong> In LuCI, open <b>Network → Firewall → Zones → WAN</b> and enable zone logging. Then opt in for sniffnetWRT with <code>uci set sniffnetwrt.wan.firewall_logging=1 &amp;&amp; uci commit sniffnetwrt &amp;&amp; /etc/init.d/sniffnetwrt restart</code>. Use a WAN log limit such as <code>10/minute</code>. This reports rejects; silent drops remain invisible.';
  }
  els.secTotal.textContent = events.length;
  els.securityNavCount.textContent = events.length;
  els.secSources.textContent = new Set(events.map((event) => event.source_ip)).size;
  const sourceCounts = countBy(events, (event) => event.source_ip);
  const topSource = mostCommon(sourceCounts);
  els.secTopSource.textContent = topSource?.[0] || '—';
  els.secTopSourceSub.textContent = topSource ? `${topSource[1]} rejected packets` : 'No events yet';
  const portCounts = countBy(events, (event) => event.destination_port ? `${event.destination_port}/${event.protocol}` : 'unknown');
  const topPort = mostCommon(portCounts);
  els.secTopPort.textContent = topPort?.[0] || '—';
  els.secTopPortSub.textContent = topPort ? `${topPort[1]} rejected packets` : 'No events yet';

  const q = els.securitySearch.value.trim().toLowerCase();
  const target = els.securityTarget.value;
  const filtered = events.filter((event) => {
    if (target && event.target !== target) return false;
    const haystack = `${event.source_ip} ${event.source_port || ''} ${event.destination_ip} ${event.destination_port || ''} ${event.protocol} ${event.target} ${event.country || ''}`.toLowerCase();
    return !q || haystack.includes(q);
  }).slice().reverse();
  els.securityCount.textContent = `${filtered.length} shown · ${events.length} in RAM`;
  if (!filtered.length) {
    els.securityBody.innerHTML = `<tr><td colspan="7" class="empty">${enabled ? 'No matching firewall rejections in memory.' : 'Enable WAN zone logging to collect rejection events.'}</td></tr>`;
    return;
  }
  els.securityBody.innerHTML = filtered.map((event) => `<tr>
    <td>${timeHms(event.timestamp)}</td>
    <td class="event-endpoint">${escapeHtml(event.source_ip)}${event.source_port ? `:${event.source_port}` : ''}</td>
    <td>${countryBadge(event.country)}</td>
    <td><span class="target-pill ${event.target === 'router' ? 'router' : 'forwarded'}">${event.target === 'router' ? 'Router' : 'LAN forward'}</span></td>
    <td class="event-endpoint">${escapeHtml(event.destination_ip)}${event.destination_port ? `:${event.destination_port}` : ''}</td>
    <td>${escapeHtml(event.protocol.toUpperCase())}</td>
    <td><span class="verdict-pill">REJECT</span></td>
  </tr>`).join('');
}
function countBy(items, keyFn) {
  const counts = new Map();
  for (const item of items) { const key = keyFn(item); counts.set(key, (counts.get(key) || 0) + 1); }
  return counts;
}
function mostCommon(counts) {
  return [...counts.entries()].sort((a, b) => b[1] - a[1])[0] || null;
}
els.securitySearch.addEventListener('input', () => renderSecurity(state.last || {}));
els.securityTarget.addEventListener('change', () => renderSecurity(state.last || {}));
els.securityRetention.value = localStorage.getItem('sniffnetwrt.securityRetention') || '3600';
els.securityRetention.addEventListener('change', () => {
  localStorage.setItem('sniffnetwrt.securityRetention', els.securityRetention.value);
  renderSecurity(state.last || {});
});
els.clearSecurity.addEventListener('click', () => {
  const url = `/security/clear${TOKEN ? `?token=${encodeURIComponent(TOKEN)}` : ''}`;
  fetch(url, { method: 'POST' }).then((response) => {
    if (!response.ok) throw new Error('Could not clear the in-memory event list.');
    if (state.last) state.last.security_events = [];
    renderSecurity(state.last || {});
  }).catch((error) => {
    els.securityNotice.classList.remove('hidden');
    els.securityNotice.textContent = error.message;
  });
});
function csvCell(value) {
  let text = String(value ?? '');
  if (/^[=+\-@]/.test(text)) text = `'${text}`;
  return `"${text.replace(/"/g, '""')}"`;
}
function exportCsv(filename, headers, records) {
  const csv = [headers, ...records].map((row) => row.map(csvCell).join(',')).join('\r\n');
  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url; link.download = filename; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function filteredSecurityEvents() {
  const cutoff = Date.now() / 1000 - Number(els.securityRetention.value || 3600);
  const q = els.securitySearch.value.trim().toLowerCase();
  return currentSecurityEvents().filter((event) => {
    if (Number(event.timestamp) < cutoff) return false;
    if (els.securityTarget.value && event.target !== els.securityTarget.value) return false;
    const text = `${event.source_ip} ${event.source_port || ''} ${event.destination_ip} ${event.destination_port || ''} ${event.protocol} ${event.target} ${event.country || ''}`.toLowerCase();
    return !q || text.includes(q);
  });
}
els.exportConnections.addEventListener('click', () => {
  const records = filteredConnections().map((c) => [c.src_ip, c.src_port, c.dst_ip, c.dst_port, els.hideDomains.checked ? '' : (c.domain || ''), c.country || '', c.asn || '', c.protocol, c.service, c.category, c.direction, c.bytes, c.packets, c.first_seen, c.last_seen]);
  exportCsv('sniffnetwrt-connections.csv', ['source_ip', 'source_port', 'destination_ip', 'destination_port', 'domain', 'country', 'asn', 'protocol', 'service', 'category', 'direction', 'bytes', 'packets', 'first_seen_unix', 'last_seen_unix'], records);
});
els.exportSecurity.addEventListener('click', () => {
  const records = filteredSecurityEvents().map((event) => [new Date(event.timestamp * 1000).toISOString(), event.source_ip, event.source_port || '', event.country || '', event.target, event.destination_ip, event.destination_port || '', event.protocol, 'REJECT']);
  exportCsv('sniffnetwrt-firewall-rejections.csv', ['time_utc', 'source_ip', 'source_port', 'country', 'target', 'destination_ip', 'destination_port', 'protocol', 'verdict'], records);
});
els.hideDomains.checked = localStorage.getItem('sniffnetwrt.hideDomains') === 'true';
els.hideDomains.addEventListener('change', () => {
  localStorage.setItem('sniffnetwrt.hideDomains', String(els.hideDomains.checked));
  renderTable();
});
function cmpConn(a, b) {
  const k = state.sortKey;
  let r = 0;
  if (typeof a[k] === 'number') r = a[k] - b[k];
  else r = String(a[k]).localeCompare(String(b[k]));
  return state.sortAsc ? r : -r;
}

// Expand connection details on row click.
els.connBody.addEventListener('click', (e) => {
  const row = e.target.closest('tr[data-id]');
  if (row) {
    const detail = row.nextElementSibling;
    if (detail && detail.classList.contains('detail-row')) {
      const wasHidden = detail.classList.contains('hidden');
      els.connBody.querySelectorAll('tr.detail-row').forEach((r) => r.classList.add('hidden'));
      if (wasHidden) detail.classList.remove('hidden');
    }
  }
});

// ---------------------------------------------------------------------------
// Filters / sorting wiring
// ---------------------------------------------------------------------------
els.search.addEventListener('input', (e) => { state.search = e.target.value; renderTable(); });
els.dirFilter.addEventListener('change', (e) => { state.dirFilter = e.target.value; renderTable(); });
els.catFilter.addEventListener('change', (e) => { state.catFilter = e.target.value; renderTable(); });

document.querySelectorAll('#tab-connections th[data-sort]').forEach((th) => {
  th.addEventListener('click', () => {
    const k = th.dataset.sort;
    if (state.sortKey === k) state.sortAsc = !state.sortAsc;
    else { state.sortKey = k; state.sortAsc = false; }
    renderTable();
  });
});

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------
document.querySelectorAll('#tabs .tab').forEach((btn) => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('#tabs .tab').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    document.querySelectorAll('.tab-panel').forEach((p) => p.classList.add('hidden'));
    document.getElementById('tab-' + btn.dataset.tab).classList.remove('hidden');
    chart.resize(); chart.draw();
  });
});

// ---------------------------------------------------------------------------
// Themes
// ---------------------------------------------------------------------------
function applyTheme(t) {
  state.theme = t;
  document.body.dataset.theme = t;
  localStorage.setItem('sniffnetwrt.theme', t);
  document.querySelectorAll('.theme-btn').forEach((b) => b.classList.toggle('active', b.dataset.themeChoice === t));
}
document.querySelectorAll('.theme-btn').forEach((b) => b.addEventListener('click', () => applyTheme(b.dataset.themeChoice)));

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------
let audioCtx = null;
function beep() {
  try {
    audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
    const o = audioCtx.createOscillator(), g = audioCtx.createGain();
    o.type = 'sine'; o.frequency.value = 880; g.gain.value = 0.08;
    o.connect(g); g.connect(audioCtx.destination); o.start(); o.stop(audioCtx.currentTime + 0.18);
  } catch (e) { /* ignore */ }
}
function notify(title, body) {
  beep();
  if (Notification.permission === 'granted') { try { new Notification(title, { body }); } catch (e) {} }
}
els.notifyEnabled.addEventListener('change', () => {
  localStorage.setItem('sniffnetwrt.notifyEnabled', String(els.notifyEnabled.checked));
  if (els.notifyEnabled.checked && Notification.permission === 'default') Notification.requestPermission();
});
for (const [element, key] of [[els.alertNewDevice, 'alertNewDevice'], [els.alertRateSpike, 'alertRateSpike']]) {
  element.checked = localStorage.getItem(`sniffnetwrt.${key}`) === 'true';
  element.addEventListener('change', () => localStorage.setItem(`sniffnetwrt.${key}`, String(element.checked)));
}
els.notifyEnabled.checked = localStorage.getItem('sniffnetwrt.notifyEnabled') === 'true';
els.notifyThreshold.value = localStorage.getItem('sniffnetwrt.notifyThreshold') || '0';
els.notifyThreshold.addEventListener('change', () => localStorage.setItem('sniffnetwrt.notifyThreshold', els.notifyThreshold.value));
const savedUpdateInterval = [1, 2, 5, 10].includes(state.updateInterval) ? state.updateInterval : 1;
state.updateInterval = savedUpdateInterval;
els.updateInterval.value = String(savedUpdateInterval);
els.updateInterval.addEventListener('change', () => {
  state.updateInterval = Number(els.updateInterval.value) || 1;
  localStorage.setItem('sniffnetwrt.updateInterval', String(state.updateInterval));
  if (state.renderTimer) clearTimeout(state.renderTimer);
  state.renderTimer = null;
  if (state.pendingSnapshot) scheduleSnapshot(state.pendingSnapshot);
});
function checkNotifications(snap) {
  const mbps = parseFloat(els.notifyThreshold.value);
  const thrBytes = (mbps || 0) * 125000;
  const total = (snap.totals?.incoming_rate || 0) + (snap.totals?.outgoing_rate || 0);
  const now = Date.now();
  const enabled = els.notifyEnabled.checked;

  // threshold
  if (enabled && mbps > 0 && total > thrBytes && now - state.lastNotify > 30000) {
    state.lastNotify = now;
    notify('sniffnetWRT', `Throughput ${fmtRate(total)} exceeds ${mbps} Mbps`);
  }

  // Newly observed online LAN devices. Seed once so enabling alerts later does not
  // announce every device that was already present.
  const devices = snap.devices || [];
  const deviceIds = devices.filter((d) => d.online).map((d) => d.mac || d.ip);
  if (state.knownDeviceIds == null) state.knownDeviceIds = new Set(deviceIds);
  else {
    const added = deviceIds.filter((id) => !state.knownDeviceIds.has(id));
    if (enabled && els.alertNewDevice.checked && added.length && now - state.lastDeviceAlert > 30000) {
      state.lastDeviceAlert = now;
      const names = devices.filter((d) => added.includes(d.mac || d.ip)).map((d) => d.hostname || d.ip).slice(0, 3).join(', ');
      notify('New LAN device', names || 'A new device appeared on your network');
    }
    for (const id of deviceIds) state.knownDeviceIds.add(id);
  }

  // Alert only after a small baseline has formed, and limit repeat alerts.
  if (enabled && els.alertRateSpike.checked && state.rateSampleCount >= 8 && total > 6_250_000 && state.rateBaseline > 0 && total > state.rateBaseline * 3 && now - state.lastSpikeAlert > 60000) {
    state.lastSpikeAlert = now;
    notify('Traffic spike detected', `${fmtRate(total)} · over 3× the recent baseline`);
  }
  state.rateBaseline = state.rateBaseline == null ? total : ema(state.rateBaseline, total, 0.08);
  state.rateSampleCount++;
}

// ---------------------------------------------------------------------------
// WebSocket + demo fallback
// ---------------------------------------------------------------------------
function setStatus(live) {
  els.status.classList.toggle('live', live);
  els.status.classList.toggle('offline', !live);
  els.statusText.textContent = live ? 'live' : 'offline';
}
function connect() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const url = `${proto}://${location.host}${WS_PATH}${TOKEN ? '?token=' + encodeURIComponent(TOKEN) : ''}`;
  let ws;
  try { ws = new WebSocket(url); } catch (e) { startDemo(); return; }
  let gotData = false;
  const failTimer = setTimeout(() => { if (!gotData) { ws.close(); startDemo(); } }, 2500);
  ws.onopen = () => setStatus(false);
  ws.onmessage = (ev) => {
    gotData = true; clearTimeout(failTimer);
    try {
      const snap = JSON.parse(ev.data);
      scheduleSnapshot(snap);
    } catch (e) {}
  };
  ws.onerror = () => {};
  ws.onclose = () => { clearTimeout(failTimer); if (!gotData) startDemo(); else setStatus(false); };
}

function startDemo() {
  setStatus(false); els.statusText.textContent = 'demo'; els.status.classList.add('live');
  els.iface.textContent = 'wan (demo)';
  const services = ['https', 'http', 'dns', 'ssh', 'quic', 'mdns', 'ntp'];
  const cats = ['Web', 'Email', 'Discovery', 'Remote', 'Network'];
  const devs = ['iPhone', 'MacBook', 'Pixel 8', 'Smart TV', 'NAS', 'Printer'];
  let inB = 4e9, outB = 5e8, inP = 3e6, outP = 4e5, boot = Date.now() - 36e5 * 9;
  const demoRejects = [
    { source_ip: '185.220.101.14', source_port: 49218, destination_ip: '203.0.113.42', destination_port: 22, protocol: 'tcp', target: 'router', country: 'DE' },
    { source_ip: '45.95.147.33', source_port: 53301, destination_ip: '192.168.1.20', destination_port: 443, protocol: 'tcp', target: 'forwarded', country: 'NL' },
    { source_ip: '91.240.118.172', source_port: 41002, destination_ip: '203.0.113.42', destination_port: 23, protocol: 'tcp', target: 'router', country: 'RU' },
    { source_ip: '198.98.51.189', source_port: 39872, destination_ip: '203.0.113.42', destination_port: 8080, protocol: 'tcp', target: 'router', country: 'US' },
  ];
  setInterval(() => {
    const inRate = (20 + Math.random() * 90) * 1e6;
    const outRate = (2 + Math.random() * 25) * 1e6;
    inB += inRate; outB += outRate; inP += inRate / 900; outP += outRate / 900;
    const conns = [];
    for (let i = 0; i < 18 + Math.floor(Math.random() * 10); i++) {
      const svc = services[Math.floor(Math.random() * services.length)];
      const bytes = Math.floor(Math.random() * 800e6);
      const dir = Math.random() < 0.6 ? 'incoming' : 'outgoing';
      conns.push({
        id: 'demo' + i, src_ip: randIp(), src_port: randPort(), dst_ip: randIp(), dst_port: randPort(),
        protocol: svc === 'dns' || svc === 'ntp' || svc === 'mdns' ? 'udp' : 'tcp',
        direction: dir, service: svc, category: cats[Math.floor(Math.random() * cats.length)],
        bytes, packets: Math.floor(bytes / 800),
        incoming_bytes: dir === 'incoming' ? bytes : 0, outgoing_bytes: dir === 'outgoing' ? bytes : 0,
        first_seen: Date.now() / 1000 - 60, last_seen: Date.now() / 1000,
        country: ['US', 'DE', 'NL', 'GB', 'JP', '?'][Math.floor(Math.random() * 6)],
        asn: Math.random() < 0.5 ? ['Cloudflare, Inc.', 'Google LLC', 'Akamai Technologies'][Math.floor(Math.random() * 3)] : null,
        domain: Math.random() < 0.5 ? ['github.com', 'cloudflare.com', 'google.com'][Math.floor(Math.random() * 3)] : null,
      });
    }
    const svcAgg = services.map((name) => ({ name, bytes: Math.floor(Math.random() * 2e9), connections: Math.floor(Math.random() * 8) }));
    scheduleSnapshot({
      type: 'tick', interface: 'wan', timestamp: Date.now() / 1000,
      totals: { incoming_bytes: inB, outgoing_bytes: outB, incoming_packets: inP, outgoing_packets: outP, incoming_rate: inRate, outgoing_rate: outRate },
      connections: conns, services: svcAgg,
      health: { load1: Math.random() * 2, load5: Math.random() * 2, load15: Math.random() * 2, mem_total_kb: 2e6, mem_used_kb: 6e5 + Math.random() * 4e5, uptime_secs: (Date.now() - boot) / 1000, temp_c: 48 + Math.random() * 12 },
      wan: { ip: '203.0.113.42', device: 'eth2', uptime_secs: (Date.now() - boot) / 1000, rx_bytes: inB, tx_bytes: outB },
      devices: devs.map((name, i) => ({ mac: `94:83:c4:${(i * 3 + 10).toString(16)}:${(i * 7 + 20).toString(16)}:${(i * 5 + 30).toString(16)}`, ip: `192.168.1.${100 + i}`, hostname: name, online: Math.random() < 0.8, incoming_bytes: Math.floor(inB * (i + 1) / devs.length), outgoing_bytes: Math.floor(outB * (i + 1) / devs.length) })),
      security_logging: true,
      security_events: demoRejects.map((event, i) => ({ ...event, timestamp: Date.now() / 1000 - i * 73 })),
    });
  }, 1000);
}
function randIp() { return `${rand(10, 254)}.${rand(1, 254)}.${rand(1, 254)}.${rand(1, 254)}`; }
function randPort() { return rand(1024, 65535); }
function rand(a, b) { return Math.floor(Math.random() * (b - a + 1)) + a; }

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
applyTheme(state.theme);
chart.resize(); chart.draw();
connect();
