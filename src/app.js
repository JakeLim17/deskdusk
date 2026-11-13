// 화면에 실제 사용량을 연결한다. Tauri 안에서는 Rust가 읽은 로컬 로그 값을, 브라우저 미리보기에서는 예시 값을 쓴다.
const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
const EN = window.LANG === 'en';
const DAYS = EN ? ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] : ['일', '월', '화', '수', '목', '금', '토'];
const PROVIDERS = [
  { key: 'claude', name: 'Claude', color: '#ffa37d', source: 'Claude Code' },
  { key: 'codex', name: 'Codex', color: '#6fe3b8', source: 'Codex CLI' },
  { key: 'gemini', name: 'Gemini', color: '#8fc0ff', source: 'Gemini CLI', unsupported: true },
  { key: 'grok', name: 'Grok', color: '#d7d3e8', source: 'Grok CLI', unsupported: true },
];
let latest = null;
// 화면에 남길 서비스. null이면 아직 고른 적이 없는 상태로, 이 컴퓨터에서 기록이 발견된 서비스만 보여준다.
let selected = null;

function mockUsage() {
  const now = Math.floor(Date.now() / 1000);
  let v = 0.4;
  const rhythm = Array.from({ length: 60 }, () => {
    v = Math.max(0.05, Math.min(1, v + (Math.random() - 0.5) * 0.5));
    return Math.round(v * 3e6);
  });
  return {
    generated_at: now,
    claude: { found: true, plan: 'max', primary: { used_percent: 62, window_minutes: 300, resets_at: now + 6480 }, secondary: { used_percent: 41, window_minutes: 10080, resets_at: now + 266400 }, limits_observed_at: now - 60, cost_today: 352.4, today: { input: 31e6, cache_read: 720e6, cache_write: 40e6, output: 1.9e6 } },
    codex: { found: true, plan: 'pro', primary: null, secondary: { used_percent: 27, window_minutes: 10080, resets_at: now + 198000 }, limits_observed_at: now - 900, cost_today: 13.7, today: { input: 25.8e6, cache_read: 202e6, cache_write: 0, output: 1e6 } },
    gemini: { found: false, plan: null, primary: null, secondary: null, limits_observed_at: null, today: { input: 0, cache_read: 0, cache_write: 0, output: 0 } },
    grok: { found: false, plan: null, primary: null, secondary: null, limits_observed_at: null, today: { input: 0, cache_read: 0, cache_write: 0, output: 0 } },
    daily: Array.from({ length: 7 }, (_, k) => {
      const c = [310, 1150, 270, 1060, 0, 40, 880][k] * 1e6, x = [32, 34, 0, 16, 0, 5, 228][k] * 1e6;
      return { start: now - (now % 86400) - (6 - k) * 86400, claude: c, codex: x, cost_claude: c * 4e-7, cost_codex: x * 6e-8 };
    }),
    unpriced: ['gpt-6-astra'],
    pricing_as_of: '2026-11-10',
    rhythm,
    agents: { claude: 2, codex: 1 },
    projects: [
      { name: 'sketchPad', total: 520e6, cache: 505e6, claude: 520e6, codex: 0 },
      { name: 'YogaFlow', total: 310e6, cache: 300e6, claude: 120e6, codex: 190e6 },
      { name: 'DeskDusk', total: 150e6, cache: 142e6, claude: 150e6, codex: 0 },
    ],
  };
}

const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

// 반올림 때문에 「1000K」「1000.0M」처럼 보이지 않도록 경계를 반올림 기준에 맞춘다.
function fmtTokens(n) {
  if (n >= 999.995e9) return (n / 1e12).toFixed(2) + 'T';
  if (n >= 999.95e6) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 999.5e3) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 999.5) return Math.round(n / 1e3) + 'K';
  return String(Math.round(n));
}

// 한도 %: 아직 다 차지 않았으면 반올림으로 100%가 되지 않게 한다.
const fmtPct = (used) => (used >= 100 ? Math.round(used) : Math.min(99, Math.round(used))) + '%';

function fmtLeft(sec) {
  if (sec < 60) return EN ? '<1 min' : '1분 안';
  const d = Math.floor(sec / 86400), h = Math.floor((sec % 86400) / 3600), m = Math.floor((sec % 3600) / 60);
  if (d) return EN ? `${d}d ${h}h` : `${d}일 ${h}시간`;
  if (h) return EN ? `${h}h ${m}m` : `${h}시간 ${m}분`;
  return EN ? `${m}m` : `${m}분`;
}

function fmtWhen(sec) {
  const d = new Date(sec * 1000), now = new Date();
  const h = d.getHours(), m = d.getMinutes();
  const same = d.toDateString() === now.toDateString();
  if (EN) {
    const t = `${h % 12 || 12}${m ? ':' + String(m).padStart(2, '0') : ''} ${h < 12 ? 'AM' : 'PM'}`;
    return same ? t : `${DAYS[d.getDay()]} ${t}`;
  }
  const t = `${h < 12 ? '오전' : '오후'} ${h % 12 || 12}시${m ? ` ${m}분` : ''}`;
  return same ? t : `${DAYS[d.getDay()]}요일 ${t}`;
}

function fmtAgo(sec) {
  if (sec < 90) return EN ? 'just now' : '방금';
  if (sec < 3600) return EN ? `${Math.round(sec / 60)} min ago` : `${Math.round(sec / 60)}분 전`;
  if (sec < 86400) return EN ? `${Math.round(sec / 3600)} hr ago` : `${Math.round(sec / 3600)}시간 전`;
  return EN ? `${Math.round(sec / 86400)} d ago` : `${Math.round(sec / 86400)}일 전`;
}

function windowLabel(mins) {
  if (mins === 300) return EN ? '5-hour' : '5시간';
  if (mins === 10080) return EN ? 'weekly' : '주간';
  if (!(mins > 0)) return EN ? 'limit' : '한도';
  if (mins >= 1440) return EN ? `${Math.round(mins / 1440)}-day` : `${Math.round(mins / 1440)}일`;
  if (mins >= 60) return EN ? `${Math.round(mins / 60)}-hour` : `${Math.round(mins / 60)}시간`;
  return EN ? `${mins}-min` : `${mins}분`;
}

// 추정 비용(달러)
function fmtCost(n) {
  if (!(n > 0)) return '$0';
  if (n < 100) return '$' + n.toFixed(2);
  return '$' + Math.round(n).toLocaleString('en-US');
}

// 모델별 한도처럼 이름이 붙은 창은 「주간 · Fable」처럼 부른다.
const winLabel = (w) => (w.label ? `${windowLabel(w.window_minutes)} · ${w.label}` : windowLabel(w.window_minutes));

// 설정에서 고른 서비스만 화면에 남긴다.
function selection() {
  // 위젯에서 서비스를 따로 골랐으면 그것을 쓴다.
  if (WIDGET && WOPTS.services) return WOPTS.services;
  if (selected) return selected;
  const found = PROVIDERS.filter((p) => latest && latest[p.key] && latest[p.key].found).map((p) => p.key);
  return found.length ? found : PROVIDERS.map((p) => p.key);
}
const shown = (key) => selection().includes(key);

// 서비스별 한도 창을 한 줄 목록으로 편다. 초기화 시각이 지난 값은 낡은 것으로 표시한다.
function allWindows(data) {
  const now = Date.now() / 1000, out = [];
  for (const p of PROVIDERS) {
    const d = data[p.key];
    if (!d || !shown(p.key)) continue;
    for (const w of [d.primary, d.secondary, ...(d.more || [])]) {
      if (!w) continue;
      out.push({ provider: p, win: w, label: winLabel(w), observedAt: d.limits_observed_at || data.generated_at, expired: w.resets_at > 0 && w.resets_at <= now });
    }
  }
  return out;
}

function renderProviders(data) {
  const now = Date.now() / 1000;
  $('provs').innerHTML = PROVIDERS.filter((p) => shown(p.key)).map((p) => {
    const d = data[p.key] || {};
    const tag = !d.found ? T('미연결') : p.unsupported ? T('준비 중') : d.plan ? esc(d.plan.charAt(0).toUpperCase() + d.plan.slice(1)) : '';
    const head = `<div class="row"><span class="name"><span class="sw" style="background:${p.color}"></span>${p.name}${tag ? ` <span class="tag">${tag}</span>` : ''}</span><span class="mut sm">${d.found ? (d.limits_source === 'direct' ? T('서버에서 직접') : p.source) : ''}</span></div>`;
    let body = '';
    if (!d.found) {
      body = `<div class="mut sm" style="margin-top:.4em">${T('{0}를 설치하면 여기에 표시돼요.', p.source)}</div>`;
    } else if (p.unsupported) {
      body = `<div class="mut sm" style="margin-top:.4em">${T('{0} 사용량은 아직 읽지 못해요.', p.name)}</div>`;
    } else {
      const wins = [d.primary, d.secondary, ...(d.more || [])].filter(Boolean);
      body = wins.map((w) => {
        const expired = w.resets_at > 0 && w.resets_at <= now;
        const known = w.resets_at > 0;
        const pct = expired ? '–' : fmtPct(w.used_percent);
        const left = expired ? T('초기화됐어요. 다음 사용 때 갱신돼요') : known ? T('초기화까지 {0}', fmtLeft(w.resets_at - now)) : T('초기화 시각 정보가 없어요');
        return `<div class="row sm" style="margin-top:.5em"><span>${esc(winLabel(w))}</span><b>${pct}</b></div>
          <div class="bar"><i style="width:${expired ? 0 : Math.min(100, w.used_percent)}%;background:${p.color}"></i></div>
          <div class="row sm mut"><span>${left}</span><span>${expired || !known ? '' : fmtWhen(w.resets_at)}</span></div>`;
      }).join('');
      const today = d.today ? d.today.input + d.today.cache_read + d.today.cache_write + d.today.output : 0;
      if (!wins.length) {
        const hint = p.key === 'claude' ? ` <span class="wrapok">${T('설정의 「연결 상태」에서 연결할 수 있어요.')}</span>` : '';
        body = `<div class="mut sm wrapok" style="margin-top:.4em">${T('한도 값이 아직 없어요. 오늘 {0} 토큰', fmtTokens(today))}${hint}</div>`;
      } else if (d.limits_observed_at) {
        body += `<div class="mut sm" style="margin-top:.35em">${T('{0} 기준 · 오늘 {1} 토큰', fmtAgo(now - d.limits_observed_at), fmtTokens(today))}</div>`;
      }
      if (d.limits_note) body += `<div class="sm wrapok" style="margin-top:.3em;color:#ffd9a8">${esc(T(d.limits_note))}</div>`;
    }
    return `<div class="prov" data-p="${p.key}">${head}${body}</div>`;
  }).join('');
}
window.applyFilter = () => { paintFilter(); if (latest) render(latest); };

function renderToday(data) {
  const sum = { input: 0, cache: 0, output: 0 };
  for (const p of PROVIDERS) {
    const t = data[p.key] && data[p.key].today;
    if (!t || !shown(p.key)) continue;
    sum.input += t.input;
    sum.cache += t.cache_read + t.cache_write;
    sum.output += t.output;
  }
  const total = sum.input + sum.cache + sum.output;
  $('total').textContent = fmtTokens(total);
  $('t-in').textContent = fmtTokens(sum.input);
  $('t-cache').textContent = fmtTokens(sum.cache);
  $('t-out').textContent = fmtTokens(sum.output);
  const pct = (n) => (total ? (n / total) * 100 : 0).toFixed(1);
  $('mix').innerHTML = `<i style="width:${pct(sum.input)}%;background:#ffd08a"></i><i style="width:${pct(sum.cache)}%;background:#9fc6ff"></i><i style="width:${pct(sum.output)}%;background:#ff9bb3"></i>`;
  const recent = data.rhythm.slice(-5).reduce((a, b) => a + b, 0) / 5;
  $('rate').textContent = recent > 0 ? T('분당 {0}', fmtTokens(recent)) : T('쉬는 중');
  const cost = PROVIDERS.reduce((n, p) => n + (shown(p.key) && data[p.key] ? data[p.key].cost_today || 0 : 0), 0);
  $('cost').textContent = fmtCost(cost);
  return { total, perMinute: recent };
}

// 최근 7일: 날마다 Claude·Codex를 쌓은 막대. 화면에 남긴 서비스만 더한다.
function renderWeek(days) {
  days = days || [];
  const rows = days.map((d) => ({
    start: d.start,
    claude: shown('claude') ? d.claude : 0,
    codex: shown('codex') ? d.codex : 0,
    cost: (shown('claude') ? d.cost_claude || 0 : 0) + (shown('codex') ? d.cost_codex || 0 : 0),
  }));
  const max = Math.max(1, ...rows.map((r) => r.claude + r.codex));
  $('wk-bars').innerHTML = rows.map((r, i) => {
    const day = new Date(r.start * 1000), last = i === rows.length - 1;
    const h = (n) => ((n / max) * 100).toFixed(1);
    const tip = `${day.getMonth() + 1}/${day.getDate()} · ${fmtTokens(r.claude + r.codex)} · ${fmtCost(r.cost)}`;
    return `<div class="wkc${last ? ' now' : ''}" title="${esc(tip)}"><div class="wkb"><i style="height:${h(r.codex)}%;background:#6fe3b8"></i><i style="height:${h(r.claude)}%;background:#ffa37d"></i></div><span>${last ? T('오늘') : DAYS[day.getDay()]}</span></div>`;
  }).join('');
  const sum = rows.reduce((a, r) => a + r.claude + r.codex, 0), cost = rows.reduce((a, r) => a + r.cost, 0);
  $('wk-sum').textContent = `${T('7일 {0}', fmtTokens(sum))} · ${T('API 환산 {0}', fmtCost(cost))}`;
  $('wk-sum').title = T('구독료가 아니에요. 같은 양을 API로 썼다면 들었을 정가예요.');
}

// 한도 경고 알림: 80%, 95%를 넘을 때 한 번씩. 설정에서 켰을 때만 보낸다(기본 꺼짐).
let alertsOn = false, alerted = {};
try { alertsOn = localStorage.getItem('tw-alerts') === '1'; alerted = JSON.parse(localStorage.getItem('tw-alerted') || '{}') || {}; } catch (e) {}
function dueAlerts(wins) {
  const now = Date.now() / 1000, out = [];
  for (const w of wins) {
    if (w.expired) continue;
    const pct = w.win.used_percent, level = pct >= 95 ? 95 : pct >= 80 ? 80 : 0;
    if (!level) continue;
    const key = `${w.provider.key}:${w.win.window_minutes}`, prev = alerted[key];
    // 같은 주기(전에 알린 초기화 시각이 아직 안 지남)에서 같은 단계는 다시 알리지 않는다.
    if (prev && prev.reset > now && prev.level >= level) continue;
    alerted[key] = { reset: w.win.resets_at, level };
    out.push({
      title: T('{0} {1} 한도가 {2}%를 넘었어요', w.provider.name, w.label, level),
      body: T('지금 {0} · 초기화까지 {1}', fmtPct(pct), fmtLeft(w.win.resets_at - now)),
    });
  }
  if (out.length) try { localStorage.setItem('tw-alerted', JSON.stringify(alerted)); } catch (e) {}
  return out;
}

function renderRhythm(buckets) {
  const max = Math.max(1, ...buckets), n = buckets.length;
  const d = 'M' + buckets.map((b, i) => `${((i * 400) / (n - 1)).toFixed(1)} ${(86 - (b / max) * 78).toFixed(1)}`).join('L');
  $('line').setAttribute('d', d);
  $('area').setAttribute('d', d + 'L400 90L0 90Z');
}

// 지금까지의 평균 속도가 이어진다고 보고 한도에 닿는 시각을 추정한다.
function renderPace(wins) {
  const now = Date.now() / 1000;
  const live = wins.filter((w) => !w.expired && w.win.resets_at > 0);
  const pctList = () => live.map((w) => `${w.provider.name} ${w.label} ${fmtPct(w.win.used_percent)}`).join(' · ');
  const warn = (label) => {
    tag.textContent = label;
    tag.style.cssText = 'background:rgba(255,190,110,.3);color:#ffe2b8';
  };
  // 이미 다 찬 한도가 있으면 예측보다 먼저 알린다(초기화가 가장 늦은 것부터).
  const full = live.filter((w) => w.win.used_percent >= 100).sort((a, b) => b.win.resets_at - a.win.resets_at)[0];
  let worst = null;
  for (const w of live) {
    const elapsed = w.win.window_minutes * 60 - (w.win.resets_at - w.observedAt);
    if (elapsed <= 0 || w.win.used_percent <= 0) continue;
    const hitAt = w.observedAt + ((100 - w.win.used_percent) / w.win.used_percent) * elapsed;
    if (hitAt < w.win.resets_at && (!worst || hitAt < worst.hitAt)) worst = { ...w, hitAt };
  }
  const tag = $('pace-tag');
  if (full) {
    warn(T('도달'));
    $('pace-main').textContent = T('{0} {1} 한도가 다 찼어요', full.provider.name, full.label);
    $('pace-sub').textContent = T('초기화까지 {0} · {1}', fmtLeft(full.win.resets_at - now), fmtWhen(full.win.resets_at));
  } else if (worst && worst.hitAt <= now) {
    // 예측 시각이 이미 지났다(값을 받은 뒤로 시간이 흐름) → 지난 시각을 보여주지 않는다.
    warn(T('주의'));
    $('pace-main').textContent = T('{0} {1} 한도가 거의 다 찼어요', worst.provider.name, worst.label);
    $('pace-sub').textContent = T('{0} 기준 {1} · 초기화까지 {2}', fmtAgo(now - worst.observedAt), fmtPct(worst.win.used_percent), fmtLeft(worst.win.resets_at - now));
  } else if (worst) {
    warn(T('주의'));
    $('pace-main').textContent = T('{0} {1} 한도가 {2}쯤 도달해요', worst.provider.name, worst.label, fmtWhen(worst.hitAt));
    $('pace-sub').textContent = T('초기화({0})보다 {1} 빨라요.', fmtWhen(worst.win.resets_at), fmtLeft(worst.win.resets_at - worst.hitAt));
  } else if (live.length) {
    tag.textContent = T('여유');
    tag.style.cssText = 'background:rgba(110,230,170,.26);color:#d6ffe9';
    $('pace-main').textContent = T('지금 속도면 한도까지 여유가 있어요');
    $('pace-sub').textContent = pctList();
  } else {
    tag.textContent = '';
    tag.style.cssText = 'display:none';
    $('pace-main').textContent = T('한도 값이 아직 없어요');
    $('pace-sub').textContent = T('CLI를 한 번 쓰면 값이 기록돼요.');
  }
}

// 100%에 가까운 비율을 반올림해 100%로 보이지 않게 한다.
function fmtShare(ratio) {
  const pct = ratio * 100;
  return (pct >= 99.5 && pct < 100 ? Math.min(99.9, pct).toFixed(1) : String(Math.round(pct))) + '%';
}

function renderSources(data) {
  const now = Date.now() / 1000;
  const when = (t) => (t ? fmtAgo(now - t) : T('값 없음'));
  const logs = T('세션 기록 · {0}', fmtAgo(now - data.generated_at));
  const rows = [
    [T('Claude 토큰'), data.claude.found ? logs : T('기록 없음')],
    [T('Claude 한도'), data.claude.limits_source === 'direct'
      ? T('서버 직접 조회 · {0}', when(data.claude.limits_observed_at))
      : data.claude.primary || data.claude.secondary ? T('상태줄 · {0}', when(data.claude.limits_observed_at)) : T('상태줄 값 없음')],
    [T('Codex 토큰'), data.codex.found ? logs : T('기록 없음')],
    [T('Codex 한도'), data.codex.primary || data.codex.secondary ? T('세션 기록 · {0}', when(data.codex.limits_observed_at)) : T('값 없음')],
    ['Gemini', data.gemini.found ? T('지원 준비 중') : T('설치 안 됨')],
    ['Grok', data.grok && data.grok.found ? T('지원 준비 중') : T('설치 안 됨')],
    [T('소진 예측'), T('지금까지 평균 속도로 추정')],
    [T('작업 중 세션'), T('최근 3분 안에 기록된 세션 수')],
    [T('API 환산 비용'), T('구독료 아님 · API 정가 기준 · 요금표 {0}', data.pricing_as_of || '–')],
    [T('비용에서 빠진 모델'), (data.unpriced || []).join(', ') || T('없음')],
  ];
  $('src-list').innerHTML = rows.map((r) => `<span>${r[0]}</span><span>${esc(r[1])}</span>`).join('');
}

function renderProjects(projects, total) {
  // 프로젝트 값은 Claude·Codex로만 나뉜다. 둘 중 하나만 골랐으면 그 서비스가 태운 양으로 다시 줄 세운다(캐시 비중은 서비스별로 나눌 수 없어 숨긴다).
  const keys = ['claude', 'codex'].filter(shown);
  const one = keys.length < 2;
  if (one) {
    projects = projects
      .map((p) => ({ ...p, total: keys.reduce((n, k) => n + (p[k] || 0), 0), cache: 0 }))
      .filter((p) => p.total > 0)
      .sort((a, b) => b.total - a.total);
  }
  const top = projects.slice(0, 3);
  if (!top.length) {
    $('projs').innerHTML = `<div class="mut" style="margin-top:.4em">${T('오늘은 아직 태운 토큰이 없어요.')}</div>`;
    $('pj-cache').textContent = '';
    return;
  }
  const max = top[0].total || 1;
  $('projs').innerHTML = top.map((p) => {
    const share = total ? Math.round((p.total / total) * 100) : 0;
    const color = p.codex > p.claude ? '#6fe3b8' : '#ffa37d';
    return `<div class="proj"><span>${esc(p.name)}</span><b>${fmtTokens(p.total)} · ${share}%</b><div class="bar"><i style="width:${((p.total / max) * 100).toFixed(1)}%;background:${color}"></i></div></div>`;
  }).join('');
  const all = projects.reduce((a, p) => a + p.total, 0), cache = projects.reduce((a, p) => a + p.cache, 0);
  $('pj-cache').textContent = all && !one ? T('캐시 비중 {0}', fmtShare(cache / all)) : '';
}

// 풍경: 한도 사용률이 시간대를, 태우는 속도가 날씨를 정한다.
const real = { level: -1, perMinute: 0 };
let previewing = false;
// 분당 10만 토큰부터 날씨가 변하기 시작해 분당 500만에서 가장 거칠어진다(로그 눈금).
const burnScale = (perMinute) => (perMinute <= 0 ? 0 : Math.max(0, Math.min(1, Math.log10(perMinute / 1e5) / Math.log10(50))));

function clockLevel() {
  // 한도 정보가 없으면 실제 시각으로 하늘을 정한다(06시 아침 → 21시 밤).
  const n = new Date(), h = n.getHours() + n.getMinutes() / 60;
  return h < 6 || h >= 21 ? 1 : (h - 6) / 15;
}

function renderScene() {
  const known = real.level >= 0;
  const words = applyScene(known ? real.level : clockLevel(), burnScale(real.perMinute));
  $('sc-main').textContent = `${words.time} · ${words.sky}`;
  $('sc-sub').textContent = known
    ? T('한도 {0} · {1}', fmtPct(real.level * 100), real.perMinute > 0 ? T('분당 {0}', fmtTokens(real.perMinute)) : T('쉬는 중'))
    : T('한도 정보가 없어 실제 시각 기준이에요');
}

// 날씨 종류를 바꿨을 때 「지금 풍경」 글자를 다시 그린다.
window.rerenderScene = () => (previewing ? renderPreview() : renderScene());

function renderPreview() {
  const l = Number($('pv-l').value), r = Number($('pv-r').value);
  const words = applyScene(l / 100, r / 100);
  $('pv-lv').textContent = l + '%';
  $('pv-rv').textContent = r === 0 ? T('쉬는 중') : T('분당 {0}', fmtTokens(1e5 * Math.pow(50, r / 100)));
  $('pv-state').textContent = `${words.time} · ${words.sky}`;
  $('sc-main').textContent = `${words.time} · ${words.sky}`;
  $('sc-sub').textContent = T('미리보기 중이에요');
}

// 연결 상태: 무엇을 읽을 수 있는지와 Claude 한도 연결. 브라우저 미리보기에서는 예시 상태를 보여준다.
let connectStatus = null;
async function loadConnect() {
  try {
    connectStatus = invoke ? await invoke('get_connect_status') : { claude_logs: true, codex_logs: true, claude_limits: 'not_connected', claude_limits_seen: false, node: true };
  } catch (e) {
    connectStatus = null;
  }
  renderConnect();
  return connectStatus;
}
function renderConnect(message) {
  const st = connectStatus;
  if (!st) return;
  const row = (cls, title, desc) => `<div class="${cls}"><i class="dot"></i><div><b>${esc(title)}</b><span class="d">${esc(desc)}</span></div></div>`;
  const lim = st.claude_limits;
  const limRow = lim === 'connected'
    ? row(st.claude_limits_seen ? 'ok' : 'warn', T('Claude 한도'), st.claude_limits_seen ? T('연결됨. 터미널에서 Claude Code를 쓸 때마다 새 값으로 바뀌어요.') : T('연결됨. 터미널에서 Claude Code를 한 번 쓰면 값이 들어와요.'))
    : lim === 'other_statusline'
      ? row('warn', T('Claude 한도'), T('다른 상태줄을 쓰고 있어 자동으로 연결하지 않아요. README의 안내대로 직접 연결할 수 있어요.'))
      : lim === 'no_claude'
        ? row('', T('Claude 한도'), T('Claude Code가 설치되어 있지 않아요.'))
        : row('warn', T('Claude 한도'), st.node ? T('아직 연결되지 않았어요. Claude Code는 한도 값을 기록에 남기지 않아서 한 번 연결이 필요해요.') : T('연결하려면 Node.js가 필요해요.'));
  $('cn-list').innerHTML = [
    row(st.claude_logs ? 'ok' : '', T('Claude 토큰'), st.claude_logs ? T('준비 없이 읽어요. Claude Code가 남긴 세션 기록에서 가져와요.') : T('Claude Code를 쓰면 여기에 표시돼요.')),
    row(st.codex_logs ? 'ok' : '', T('Codex 토큰과 한도'), st.codex_logs ? T('준비 없이 읽어요. Codex가 남긴 세션 기록에서 가져와요.') : T('Codex를 쓰면 여기에 표시돼요.')),
    limRow,
  ].join('');
  const btn = $('cn-btn');
  const can = lim === 'connected' || (lim === 'not_connected' && st.node);
  btn.style.display = can ? '' : 'none';
  btn.textContent = lim === 'connected' ? T('연결 끊기') : T('Claude 한도 연결하기');
  btn.dataset.on = lim === 'connected' ? '0' : '1';
  $('cn-msg').textContent = message || '';
  $('cn-help').textContent = lim === 'connected' || lim === 'not_connected'
    ? T('연결하면 Claude 설정 파일(~/.claude/settings.json)의 상태줄 항목 하나만 바꾸고, 바꾸기 전에 같은 폴더에 백업을 남겨요.')
    : '';
}
function setConnect(on) {
  if (on) {
    setPreview(false);
    setSources(false);
    if (window.setSettings) window.setSettings(false);
    loadConnect();
  }
  $('connect').classList.toggle('on', on);
  $('cn-open').setAttribute('aria-expanded', String(on));
}
window.setConnect = setConnect;
async function toggleClaudeConnect() {
  const on = $('cn-btn').dataset.on === '1';
  if (!invoke) {
    renderConnect(T('앱에서만 동작해요'));
    return;
  }
  $('cn-btn').disabled = true;
  try {
    connectStatus = await invoke('set_claude_connect', { on });
    renderConnect(on ? T('연결했어요. 터미널에서 Claude Code를 한 번 쓰면 값이 들어와요.') : T('연결을 끊었어요.'));
    refresh();
  } catch (e) {
    renderConnect(String(e));
  }
  $('cn-btn').disabled = false;
}

function setSources(on) {
  if (on) {
    setPreview(false);
    setConnect(false);
    if (window.setSettings) window.setSettings(false);
  }
  $('sources').classList.toggle('on', on);
  $('src-open').setAttribute('aria-expanded', String(on));
}

function setPreview(on) {
  if (on) {
    setSources(false);
    setConnect(false);
    if (window.setSettings) window.setSettings(false);
  }
  previewing = on;
  $('preview').classList.toggle('on', on);
  $('pv-open').setAttribute('aria-expanded', String(on));
  if (on) {
    $('pv-l').value = Math.round(Math.max(0, real.level) * 100);
    $('pv-r').value = Math.round(burnScale(real.perMinute) * 100);
    renderPreview();
  } else {
    renderScene();
  }
}

// 메뉴바 요약. 'each'는 화면에 남긴 서비스마다 가장 많이 찬 한도를, 'top'은 전체에서 가장 많이 찬 한도 하나와 남은 시간을 보여준다.
let trayMode = 'each', lastTray = null;
try { if (localStorage.getItem('tw-tray') === 'top') trayMode = 'top'; } catch (e) {}
// 서비스마다 메뉴바와 위젯 큰 숫자에 보일 한도. 없거나 'auto'면 가장 많이 찬 것.
// 값은 한도 길이(분)와 이름표로 정한다(예: '10080', '10080:Fable'). 고른 한도가 지금 없으면 가장 많이 찬 것으로 돌아간다.
let picks = {};
try { picks = JSON.parse(localStorage.getItem('tw-pick') || '{}') || {}; } catch (e) {}
const winKey = (w) => String(w.window_minutes) + (w.label ? ':' + w.label : '');
function picked(wins) {
  const live = wins.filter((w) => !w.expired).sort((a, b) => b.win.used_percent - a.win.used_percent);
  return PROVIDERS.map((p) => {
    const mine = live.filter((w) => w.provider.key === p.key);
    const want = picks[p.key];
    return (want && want !== 'auto' && mine.find((w) => winKey(w.win) === want)) || mine[0];
  }).filter(Boolean);
}
const pickedTop = (wins) => picked(wins).sort((a, b) => b.win.used_percent - a.win.used_percent)[0];

function trayTitle(wins, total) {
  const now = Date.now() / 1000;
  const best = picked(wins);
  if (!best.length) return T('오늘 {0}', fmtTokens(total));
  if (trayMode === 'each' && best.length > 1) return best.map((w) => `${w.provider.name} ${fmtPct(w.win.used_percent)}`).join(' · ');
  const top = pickedTop(wins);
  const left = top.win.resets_at > now ? ` · ${fmtLeft(top.win.resets_at - now)}` : '';
  return `${top.provider.name} ${fmtPct(top.win.used_percent)}${left}`;
}
// 윈도우 트레이의 숫자 아이콘에 쓸 값: 가장 많이 찬 한도와 그 서비스의 색. 한도가 없으면 null.
function trayBadge(wins) {
  const top = pickedTop(wins);
  return top ? { pct: top.win.used_percent, color: top.provider.color } : null;
}
function setTrayMode(m) {
  trayMode = m === 'top' ? 'top' : 'each';
  document.querySelectorAll('#tray-mode button').forEach((b) => b.classList.toggle('on', b.dataset.m === trayMode));
  try { localStorage.setItem('tw-tray', trayMode); } catch (e) {}
  if (invoke && lastTray) invoke('set_tray_title', { title: trayTitle(lastTray.wins, lastTray.total), badge: trayBadge(lastTray.wins) }).catch(() => {});
}

// 설정의 「메뉴바에 표시」 아래: 서비스마다 어떤 한도를 보일지 고르는 단추. 지금 읽힌 한도로 만든다.
function renderPick(wins) {
  const box = $('tray-pick');
  if (!box) return;
  const rows = PROVIDERS.map((p) => {
    const mine = wins.filter((w) => w.provider.key === p.key && !w.expired);
    if (mine.length < 2) return '';
    const cur = picks[p.key] && mine.some((w) => winKey(w.win) === picks[p.key]) ? picks[p.key] : 'auto';
    const btn = (v, label) => `<button data-p="${p.key}" data-v="${esc(v)}" class="${cur === v ? 'on' : ''}">${esc(label)}</button>`;
    return `<div class="row sm" style="margin-top:.45em;gap:.6em;justify-content:flex-start"><span style="min-width:4.5em">${p.name}</span><span class="seg" style="flex-wrap:wrap;border-radius:1em" role="group" aria-label="${esc(T('{0} 한도', p.name))}">${btn('auto', T('가장 많이 찬 것'))}${mine.map((w) => btn(winKey(w.win), winLabel(w.win))).join('')}</span></div>`;
  }).join('');
  box.innerHTML = rows ? `<div class="mut sm wrapok" style="margin-top:.5em">${T('메뉴바와 위젯의 큰 숫자에 보일 한도예요.')}</div>${rows}` : '';
}
function setPick(key, value) {
  picks[key] = value;
  try { localStorage.setItem('tw-pick', JSON.stringify(picks)); } catch (e) {}
  if (lastTray) {
    renderPick(lastTray.wins);
    if (invoke) invoke('set_tray_title', { title: trayTitle(lastTray.wins, lastTray.total), badge: trayBadge(lastTray.wins) }).catch(() => {});
  }
}

function render(data) {
  latest = data;
  paintFilter();
  const now = Date.now() / 1000;
  renderProviders(data);
  const { total, perMinute } = renderToday(data);
  renderRhythm(data.rhythm);
  renderProjects(data.projects || [], total);
  renderSources(data);
  const wins = allWindows(data);
  renderPace(wins);
  real.level = wins.filter((w) => !w.expired).reduce((m, w) => Math.max(m, w.win.used_percent / 100), -1);
  real.perMinute = perMinute;
  if (!previewing) renderScene();

  const next = wins.filter((w) => !w.expired && w.win.resets_at > now).sort((a, b) => a.win.resets_at - b.win.resets_at)[0];
  $('headline').textContent = !next
    ? T('오늘 {0} 토큰을 태웠어요', fmtTokens(total))
    : next.win.resets_at - now < 60
      ? T('{0} {1} 한도가 곧 초기화돼요', next.provider.name, next.label)
      : T('{0} {1} 한도가 {2} 뒤 초기화돼요', next.provider.name, next.label, fmtLeft(next.win.resets_at - now));
  $('upd').textContent = T('{0} 갱신', fmtAgo(now - data.generated_at));

  const agents = (data.agents.claude || 0) + (data.agents.codex || 0);
  $('agents').textContent = agents ? T('세션 {0}개 작업 중', agents) : T('쉬는 중');
  $('agdot').style.cssText = agents ? '' : 'background:#9aa0b4;box-shadow:none';

  lastTray = { wins, total };
  renderWeek(data.daily);
  if (WIDGET) {
    renderMini(wins, total, perMinute, data);
    return;
  }
  renderPick(wins);
  // 메뉴바 요약과 알림은 대시보드 창만 보낸다(위젯 창과 겹치지 않게).
  if (invoke) {
    const title = trayTitle(wins, total);
    invoke('set_tray_title', { title, badge: trayBadge(wins) }).catch(() => {});
  }
  if (alertsOn) for (const a of dueAlerts(wins)) if (invoke) invoke('notify', a).catch(() => {});
}

// 바탕화면 위젯: 가장 많이 찬 한도를 크게, 나머지는 위젯마다 고른 항목만 보여준다.
let lastMini = null;
function renderMini(wins, total, perMinute, data) {
  lastMini = { wins, total, perMinute, data };
  const now = Date.now() / 1000, has = (k) => WOPTS.items.includes(k);
  const live = wins.filter((w) => !w.expired).sort((a, b) => b.win.used_percent - a.win.used_percent);
  const top = pickedTop(wins);
  $('m-scene').textContent = has('scene') ? $('sc-main').textContent : '';
  if (top) {
    $('m-pct').textContent = fmtPct(top.win.used_percent);
    $('m-label').textContent = `${top.provider.name} ${top.label}` + (has('reset') && top.win.resets_at > now ? ` · ${T('초기화까지 {0}', fmtLeft(top.win.resets_at - now))}` : '');
  } else {
    $('m-pct').textContent = fmtTokens(total);
    $('m-label').textContent = T('오늘 태운 토큰');
  }
  $('m-bars').innerHTML = !has('bars') ? '' : live.slice(0, 4).map((w) =>
    `<div><span>${esc(w.provider.name)} ${esc(w.label)}</span><div class="bar"><i style="width:${Math.min(100, w.win.used_percent)}%;background:${w.provider.color}"></i></div><b>${fmtPct(w.win.used_percent)}</b></div>`).join('');
  // 가장 긴 이름(예: 「Claude 주간 · Fable」)에 맞춰 이름 칸 너비를 정해 막대가 어긋나지 않게 한다.
  const names = [...document.querySelectorAll('#m-bars>div>span')];
  $('m-bars').style.removeProperty('--lw');
  if (names.length) $('m-bars').style.setProperty('--lw', Math.ceil(Math.max(...names.map((e) => e.scrollWidth))) + 'px');
  const days = has('week') && data && data.daily ? data.daily.map((d) => (shown('claude') ? d.claude : 0) + (shown('codex') ? d.codex : 0)) : [];
  const max = Math.max(1, ...days);
  $('m-week').innerHTML = days.map((n) => `<i style="height:${Math.max(4, (n / max) * 100).toFixed(0)}%"></i>`).join('');
  $('m-pace').textContent = has('pace') ? $('pace-main').textContent : '';
  const parts = [];
  if (has('today')) parts.push(T('오늘 {0}', fmtTokens(total)), perMinute > 0 ? T('분당 {0}', fmtTokens(perMinute)) : T('쉬는 중'));
  if (has('cost') && data) parts.push(T('API 환산 {0}', fmtCost(PROVIDERS.reduce((n, p) => n + (shown(p.key) && data[p.key] ? data[p.key].cost_today || 0 : 0), 0))));
  $('m-sub').textContent = parts.join(' · ');
  tickClock();
  fitMini();
}

// 위젯이 작아 다 들어가지 않으면 덜 중요한 항목부터 감춘다(가장 많이 찬 한도 숫자는 끝까지 남긴다).
function fitMini() {
  const mini = $('mini');
  if (!WIDGET || WOPTS.shape === 'frame') return;
  mini.querySelectorAll('.nofit').forEach((e) => e.classList.remove('nofit'));
  const seen = (e) => e && e.getClientRects().length > 0 && (e.textContent.trim() || e.children.length);
  const topRow = mini.querySelector('.m-top > span');
  const over = () => {
    // 풍경과 숫자 모양에서는 맨 윗줄(시계·풍경 글자) 아래부터 쓸 수 있다.
    const top = WOPTS.shape === 'scene' && seen(topRow) ? topRow.getBoundingClientRect().bottom : 0;
    const wm = $('wm').getBoundingClientRect();
    return [...mini.querySelectorAll('#m-pct,#m-label,#m-bars>div,#m-week,#m-pace,#m-sub')].filter(seen).some((e) => {
      const r = e.getBoundingClientRect();
      const onMark = r.left < wm.right && wm.left < r.right && r.top < wm.bottom && wm.top < r.bottom && e.id !== 'm-sub' && e.id !== 'm-pace';
      return r.bottom > innerHeight + 0.5 || r.top < top - 0.5 || r.right > innerWidth + 0.5 || onMark;
    });
  };
  const lastBar = () => [...mini.querySelectorAll('#m-bars>div:not(.nofit)')].pop();
  const order = [() => $('m-week'), () => $('m-pace'), lastBar, lastBar, () => $('m-sub'), lastBar, lastBar, () => $('m-label'), () => $('m-scene'), () => $('m-clock')];
  for (const pick of order) {
    if (!over()) break;
    const e = pick();
    if (e) e.classList.add('nofit');
  }
}

async function refresh() {
  try {
    render(invoke ? await invoke('get_usage') : mockUsage());
  } catch (e) {
    $('headline').textContent = T('사용량을 읽지 못했어요. 잠시 뒤 다시 시도해요.');
    console.error(e);
  }
}

function tickClock() {
  const n = new Date();
  $('m-clock').textContent = WIDGET && WOPTS.items.includes('clock') ? String(n.getHours()).padStart(2, '0') + ':' + String(n.getMinutes()).padStart(2, '0') : '';
  $('clock').textContent = String(n.getHours()).padStart(2, '0') + ':' + String(n.getMinutes()).padStart(2, '0');
  $('date').textContent = EN
    ? n.toLocaleDateString('en-US', { weekday: 'long', month: 'short', day: 'numeric' })
    : `${n.getMonth() + 1}월 ${n.getDate()}일 ${DAYS[n.getDay()]}요일`;
}

$('pv-open').addEventListener('click', () => setPreview(true));
$('pv-real').addEventListener('click', () => setPreview(false));
$('src-open').addEventListener('click', () => setSources(true));
$('cn-open').addEventListener('click', () => setConnect(true));
$('cn-btn').addEventListener('click', toggleClaudeConnect);
// 선택 기능: 정확한 Claude 한도 가져오기(기본 꺼짐). 앱에서만 보인다.
if (invoke && !WIDGET) {
  const box = $('direct-on');
  $('direct-opts').style.display = '';
  invoke('get_claude_direct').then((on) => { box.checked = !!on; }).catch(() => {});
  box.addEventListener('change', async () => {
    const want = box.checked;
    box.disabled = true;
    try {
      box.checked = !!(await invoke('set_claude_direct', { on: want }));
      $('direct-msg').textContent = box.checked ? T('켰어요. 곧 새 값으로 바뀌어요.') : T('껐어요.');
      refresh();
    } catch (e) {
      box.checked = !want;
      $('direct-msg').textContent = String(e);
    }
    box.disabled = false;
  });
}
$('pv-l').addEventListener('input', renderPreview);
$('pv-r').addEventListener('input', renderPreview);
function paintFilter() {
  document.querySelectorAll('#filter button').forEach((x) => {
    const on = shown(x.dataset.p);
    x.classList.toggle('on', on);
    x.setAttribute('aria-pressed', String(on));
  });
}
// 서비스를 하나씩 켜고 끈다. 마지막 하나는 끌 수 없다.
function toggleProvider(key) {
  if (!PROVIDERS.some((p) => p.key === key)) return;
  const cur = selection();
  const next = cur.includes(key) ? cur.filter((k) => k !== key) : PROVIDERS.map((p) => p.key).filter((k) => k === key || cur.includes(k));
  if (!next.length) return;
  selected = next;
  try { localStorage.setItem('tw-filter', JSON.stringify(selected)); } catch (e) {}
  window.applyFilter();
}
$('filter').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) toggleProvider(b.dataset.p);
});
// 저장 값: 고른 서비스 목록. 예전 형식('all' 또는 서비스 하나)도 읽는다.
try {
  const raw = localStorage.getItem('tw-filter');
  const known = PROVIDERS.map((p) => p.key);
  if (raw && raw !== 'all') {
    let list = known.includes(raw) ? [raw] : JSON.parse(raw);
    list = Array.isArray(list) ? known.filter((k) => list.includes(k)) : [];
    if (list.length) selected = list;
  }
} catch (e) {}
paintFilter();
$('tray-mode').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) setTrayMode(b.dataset.m);
});
$('tray-pick').addEventListener('click', (e) => {
  const b = e.target.closest('button[data-p]');
  if (b) setPick(b.dataset.p, b.dataset.v);
});
setTrayMode(trayMode);
if (invoke) $('tray-opts').style.display = '';
// 알림(기본 꺼짐)과 언어
$('alerts').checked = alertsOn;
$('alerts').addEventListener('change', (e) => {
  alertsOn = e.target.checked;
  try { localStorage.setItem('tw-alerts', alertsOn ? '1' : '0'); } catch (x) {}
  if (alertsOn && latest) render(latest);
});
if (invoke) $('alert-opts').style.display = '';
document.querySelectorAll('#lang button').forEach((b) => b.classList.toggle('on', b.dataset.l === window.LANG));
$('lang').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (!b || b.dataset.l === window.LANG) return;
  try { localStorage.setItem('tw-lang', b.dataset.l); } catch (x) {}
  location.reload();
});
// 처음 실행: 「시작하기」 화면을 한 번 보여준다. 위젯은 본체가 이미 띄워 두었다.
function openWelcome() {
  setPreview(false);
  setSources(false);
  setConnect(false);
  if (window.setSettings) window.setSettings(false);
  $('welcome').classList.add('on');
  const bind = (boxId, getCmd, setCmd) => {
    const box = $(boxId);
    if (!invoke) return;
    invoke(getCmd).then((on) => { box.checked = !!on; }).catch(() => {});
    box.addEventListener('change', async () => {
      const want = box.checked;
      box.disabled = true;
      try { box.checked = !!(await invoke(setCmd, { on: want })); } catch (e) { box.checked = !want; }
      box.disabled = false;
      refresh();
    });
  };
  bind('wl-direct', 'get_claude_direct', 'set_claude_direct');
  bind('wl-auto', 'get_autostart', 'set_autostart');
}
$('wl-done').addEventListener('click', () => $('welcome').classList.remove('on'));
(() => {
  if (WIDGET) return;
  let seen = '1';
  try { seen = localStorage.getItem('tw-onboarded'); } catch (e) {}
  if (seen && !QS.has('welcome')) return;
  try { localStorage.setItem('tw-onboarded', '1'); } catch (e) {}
  openWelcome();
})();
if (invoke && !WIDGET) invoke('set_menu_labels', { show: T('대시보드 열기'), widget: T('바탕화면 위젯 켜고 끄기'), quit: T('종료'), top: T('위젯을 다른 창 위에 띄우기') }).catch(() => {});
// 바탕화면 위젯 창: 끌어서 옮기고, 두 번 누르거나 단추를 누르면 대시보드를 연다. 설정이 바뀌면 다시 그린다.
// 위젯 메뉴(오른쪽 클릭): 모양, 보여 줄 항목, 서비스, 풍경을 위젯마다 바꾼다.
const WIDGET_SHAPES = [['scene', '풍경과 숫자'], ['strip', '숫자 띠'], ['frame', '풍경만']];
const WIDGET_ITEMS = [['clock', '시계'], ['scene', '풍경 글자'], ['bars', '한도 막대'], ['reset', '초기화까지 남은 시간'], ['today', '오늘 토큰과 속도'], ['cost', 'API 환산 비용'], ['week', '최근 7일'], ['pace', '소진 예측']];
const WIDGET_SIZES = { scene: [440, 275], strip: [460, 92], frame: [440, 275] };
function widgetMenuSpec() {
  const themes = [...document.querySelectorAll('#themes button')].map((b) => [b.dataset.th, b.textContent]);
  const svc = PROVIDERS.filter((p) => !p.unsupported);
  return [
    { id: 'open', label: T('대시보드 열기') },
    { sep: true },
    { label: T('모양'), items: WIDGET_SHAPES.map(([k, l]) => ({ id: 'shape:' + k, label: T(l), checked: WOPTS.shape === k })) },
    { label: T('보여 줄 항목'), items: WIDGET_ITEMS.map(([k, l]) => ({ id: 'item:' + k, label: T(l), checked: WOPTS.items.includes(k) })) },
    { label: T('서비스'), items: [{ id: 'svc:auto', label: T('대시보드 따라가기'), checked: !WOPTS.services }, { sep: true }, ...svc.map((p) => ({ id: 'svc:' + p.key, label: p.name, checked: !!WOPTS.services && WOPTS.services.includes(p.key) }))] },
    { label: T('풍경'), items: [{ id: 'theme:auto', label: T('대시보드 따라가기'), checked: !WOPTS.theme }, { sep: true }, ...themes.map(([k, l]) => ({ id: 'theme:' + k, label: l, checked: WOPTS.theme === k }))] },
    { sep: true },
    { id: 'top', label: T('항상 위에 두기'), checked: WOPTS.top },
    { id: 'primary', label: T('화면 가운데로 옮기기') },
    { id: 'add', label: T('위젯 하나 더 띄우기') },
    { id: 'close', label: T('이 위젯 닫기') },
  ];
}
// 메뉴에서 고른 항목을 적용하고 기억한다. 본체가 이 함수를 부른다.
// 메뉴바 메뉴에서 모든 위젯의 높이를 바꿨을 때 본체가 알려 준다.
window.__twSetTop = (on) => { if (typeof WOPTS !== 'undefined') WOPTS.top = !!on; };
window.__twMenu = function (id) {
  const call = (name, args) => (invoke ? invoke(name, args).catch(() => {}) : Promise.resolve());
  const [kind, value] = String(id).split(':');
  if (kind === 'open') return call('open_dashboard');
  if (kind === 'primary') return call('widget_to_primary');
  if (kind === 'add') return call('add_widget');
  if (kind === 'close') return call('remove_widget');
  if (kind === 'top') {
    WOPTS.top = !WOPTS.top;
    call('widget_set_top', { on: WOPTS.top });
  } else if (kind === 'shape' && WIDGET_SIZES[value]) {
    WOPTS.shape = value;
    applyWidgetShape();
    call('widget_resize', { width: WIDGET_SIZES[value][0], height: WIDGET_SIZES[value][1] });
  } else if (kind === 'item' && WIDGET_ITEMS.some(([k]) => k === value)) {
    WOPTS.items = WOPTS.items.includes(value) ? WOPTS.items.filter((k) => k !== value) : WIDGET_ITEMS.map(([k]) => k).filter((k) => k === value || WOPTS.items.includes(k));
  } else if (kind === 'svc') {
    if (value === 'auto') WOPTS.services = null;
    else if (PROVIDERS.some((p) => p.key === value)) {
      const cur = WOPTS.services || [];
      const next = cur.includes(value) ? cur.filter((k) => k !== value) : PROVIDERS.map((p) => p.key).filter((k) => k === value || cur.includes(k));
      WOPTS.services = next.length ? next : null;
    }
  } else if (kind === 'theme') {
    WOPTS.theme = value === 'auto' ? null : value;
    let t = WOPTS.theme;
    if (!t) try { t = localStorage.getItem('tw-theme'); } catch (e) {}
    if (window.setTheme) window.setTheme(t || 'city');
  } else {
    return undefined;
  }
  call('set_widget_opts', { opts: WOPTS });
  if (latest) render(latest);
  return undefined;
};
// 바탕화면 위젯 창: 끌어서 옮기고, 두 번 누르거나 단추를 누르면 대시보드를 연다. 대시보드 설정이 바뀌면 다시 그린다.
if (WIDGET) {
  const open = () => invoke && invoke('open_dashboard').catch(() => {});
  $('m-open').addEventListener('click', open);
  $('mini').addEventListener('dblclick', open);
  $('mini').addEventListener('pointerdown', (e) => {
    if (e.button === 0 && !e.target.closest('button') && invoke) invoke('widget_drag').catch(() => {});
  });
  window.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (invoke) invoke('widget_menu', { spec: widgetMenuSpec() }).catch(() => {});
  });
  window.addEventListener('storage', (e) => { if (e.key !== 'tw-widget-tip') location.reload(); });
  window.addEventListener('resize', fitMini);
  // 처음 한 번, 위젯을 어떻게 다루는지 알려 준다. 끌거나 오른쪽 클릭하면 사라진다.
  let tipSeen = '1';
  try { tipSeen = localStorage.getItem('tw-widget-tip'); } catch (e) {}
  if (!tipSeen || QS.has('tip')) {
    $('m-tip').style.display = '';
    const hide = () => { $('m-tip').style.display = 'none'; try { localStorage.setItem('tw-widget-tip', '1'); } catch (e) {} };
    window.addEventListener('pointerdown', hide, { once: true });
    window.addEventListener('contextmenu', hide, { once: true });
    setTimeout(hide, 60000);
  }
}
// 바탕화면 위젯 켜고 끄기(앱에서만). 메뉴바에서 바꿨을 수도 있어 설정을 열 때마다 다시 읽는다.
if (invoke && !WIDGET) {
  const box = $('widget-on');
  const sync = () => invoke('get_widget').then((on) => { box.checked = !!on; }).catch(() => {});
  sync();
  $('set-open').addEventListener('click', sync);
  box.addEventListener('change', async () => {
    const want = box.checked;
    try { box.checked = !!(await invoke('set_widget', { on: want })); } catch (e) { box.checked = !want; }
  });
  $('widget-add').addEventListener('click', () => invoke('add_widget').then(sync).catch(() => {}));
}
// 로그인할 때 자동 실행: 앱에서만 보이고, 실제 상태는 본체가 알려 준다.
if (invoke) {
  const box = $('autostart');
  $('app-opts').style.display = '';
  invoke('get_autostart').then((on) => { box.checked = !!on; }).catch(() => {});
  box.addEventListener('change', async () => {
    const want = box.checked;
    box.disabled = true;
    try {
      box.checked = !!(await invoke('set_autostart', { on: want }));
    } catch (e) {
      box.checked = !want;
    }
    box.disabled = false;
  });
}
tickClock();
setInterval(tickClock, 1000);
refresh();
// 앱에서는 본체가 20초마다 __twTick 을 불러 준다. 창이 숨겨지면 화면 쪽 타이머가 멈추기 때문이다(2026-11-14 측정).
// 1분마다 새로 읽고, 그 사이에는 남은 시간 표시만 다시 그린다. 브라우저 미리보기에서는 화면 쪽 타이머를 쓴다.
window.__twTick = (full) => {
  tickClock();
  if (full) refresh();
  else if (latest) render(latest);
};
if (!invoke) {
  setInterval(refresh, 60000);
  setInterval(() => latest && render(latest), 20000);
}
