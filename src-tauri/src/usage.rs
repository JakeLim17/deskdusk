//! 로컬 로그만 읽어 사용량을 모은다. 인증 파일·키체인은 열지 않고, 네트워크 호출도 하지 않는다.
//! - Codex: ~/.codex/sessions/**/*.jsonl 의 token_count 이벤트(한도·토큰)
//! - Claude Code: ~/.claude/projects/**/*.jsonl 의 message.usage(토큰)
//! - Claude 한도 %: 상태줄 연결이 저장한 ~/.deskdusk/claude-limits.json 이 있을 때만
//!
//! 로그는 뒤에 덧붙는 파일이라, 파일마다 읽은 위치를 기억해 새로 붙은 줄만 다시 읽는다.

use chrono::{DateTime, Local, TimeZone};
use serde::Serialize;
use serde_json::Value;
use crate::pricing;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const RHYTHM_MINUTES: usize = 60;
/// 화면은 위 3개만 쓰지만, 서비스 하나만 골라 다시 줄 세울 수 있게 넉넉히 넘긴다.
const TOP_PROJECTS: usize = 20;
/// 오늘을 포함해 며칠 치 기록을 보여줄지
pub const HISTORY_DAYS: usize = 7;
const DAY: i64 = 86_400;

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct LimitWindow {
    pub used_percent: f64,
    pub window_minutes: i64,
    /// 초기화 시각(유닉스 초)
    pub resets_at: i64,
    /// 모델별 한도처럼 따로 붙는 이름(없으면 창 길이로 부른다)
    pub label: Option<String>,
}

#[derive(Serialize, Default, Clone, Debug)]
pub struct Tokens {
    /// 캐시가 아닌 새 입력
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// cache_write 가운데 1시간 캐시로 쓴 양(요금이 더 비싸다)
    pub cache_write_1h: u64,
    pub output: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.cache_read + self.cache_write + self.output
    }
    fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
        self.cache_write_1h += o.cache_write_1h;
        self.output += o.output;
    }
}

#[derive(Serialize, Default, Debug)]
pub struct Provider {
    /// 로컬 자료 폴더가 있는지
    pub found: bool,
    pub plan: Option<String>,
    /// 짧은 창(예: 5시간)
    pub primary: Option<LimitWindow>,
    /// 긴 창(예: 주간)
    pub secondary: Option<LimitWindow>,
    /// 한도 값이 기록된 시각(유닉스 초). 이후 안 썼으면 값이 낡았을 수 있다.
    pub limits_observed_at: Option<i64>,
    /// 모델별 주간 한도 등 그 밖의 창
    pub more: Vec<LimitWindow>,
    /// 한도 값을 어디서 받았는지: "direct"(서버 직접 조회) 또는 없음(로컬 기록)
    pub limits_source: Option<String>,
    /// 한도 값에 붙일 안내(직접 조회가 실패했을 때 등)
    pub limits_note: Option<String>,
    pub today: Tokens,
    /// 오늘 쓴 토큰을 API 정가로 환산한 추정 비용(달러)
    pub cost_today: f64,
}

#[derive(Serialize, Default, Debug)]
pub struct Agents {
    pub claude: u32,
    pub codex: u32,
}

/// 오늘 프로젝트(작업 폴더)별로 태운 토큰
#[derive(Serialize, Default, Debug)]
pub struct ProjectUse {
    pub name: String,
    pub total: u64,
    pub cache: u64,
    pub claude: u64,
    pub codex: u64,
}

/// 하루 치 사용량
#[derive(Serialize, Default, Clone, Debug)]
pub struct DayUse {
    /// 그날의 시작(유닉스 초)
    pub start: i64,
    pub claude: u64,
    pub codex: u64,
    /// 추정 비용(달러)
    pub cost_claude: f64,
    pub cost_codex: f64,
}

#[derive(Serialize, Default, Debug)]
pub struct Usage {
    pub generated_at: i64,
    pub codex: Provider,
    pub claude: Provider,
    pub gemini: Provider,
    /// 아직 읽지 못한다. 설치 여부만 알린다.
    pub grok: Provider,
    /// 최근 60분의 분당 토큰 합계(오래된 것부터)
    pub rhythm: Vec<u64>,
    pub agents: Agents,
    /// 오늘 많이 태운 순서
    pub projects: Vec<ProjectUse>,
    /// 최근 7일(오래된 날부터, 마지막이 오늘)
    pub daily: Vec<DayUse>,
    /// 요금표에 없어 비용에서 빠진 모델 이름
    pub unpriced: Vec<String>,
    /// 요금표 기준 날짜
    pub pricing_as_of: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Claude,
    Codex,
}

struct Event {
    ts: i64,
    tokens: Tokens,
    /// 같은 응답이 여러 줄·여러 파일에 반복 기록될 때 한 번만 세기 위한 키
    key: Option<String>,
    project: String,
    /// 요금표에서 이 모델이 몇 번째 줄인지(없으면 비용 계산에서 빠진다)
    price: Option<u16>,
}

struct Limits {
    ts: i64,
    primary: Option<LimitWindow>,
    secondary: Option<LimitWindow>,
    plan: Option<String>,
}

#[derive(Default)]
struct FileState {
    offset: u64,
    events: Vec<Event>,
    project: String,
    /// Codex 누적값(입력, 캐시, 출력)의 직전 값
    prev: (u64, u64, u64),
    limits: Option<Limits>,
    /// Codex: 지금 쓰는 모델의 요금 줄
    price: Option<u16>,
    /// 요금표에 없던 모델 이름
    unpriced: BTreeSet<String>,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, FileState>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, FileState>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn midnight() -> DateTime<Local> {
    let d = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    Local.from_local_datetime(&d).earliest().unwrap_or_else(Local::now)
}

fn parse_ts(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|d| d.timestamp())
}

fn jsonl_files(dir: &Path, out: &mut Vec<(PathBuf, SystemTime)>, depth: u8) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            jsonl_files(&p, out, depth + 1);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "jsonl") {
            if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                out.push((p, m));
            }
        }
    }
}

fn u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

fn window(v: &Value) -> Option<LimitWindow> {
    if !v.is_object() {
        return None;
    }
    Some(LimitWindow {
        used_percent: v.get("used_percent")?.as_f64()?,
        window_minutes: v.get("window_minutes").and_then(Value::as_i64).unwrap_or(0),
        resets_at: v.get("resets_at").and_then(Value::as_i64).unwrap_or(0),
        label: None,
    })
}

/// 작업 폴더 경로에서 알아보기 쉬운 프로젝트 이름을 만든다.
/// - 작업본(.claude/worktrees/…)은 원래 저장소 이름으로 묶는다.
/// - build46, mobile처럼 흔한 이름은 윗 폴더 이름을 앞에 붙인다.
fn project_name(cwd: &str) -> String {
    const GENERIC: [&str; 14] = ["mobile", "app", "apps", "src", "src-tauri", "web", "ios", "android", "frontend", "backend", "client", "server", "api", "packages"];
    // 윈도우 경로(C:\\Users\\…)도 같은 규칙으로 다루도록 구분자를 맞춘다.
    let cwd = cwd.replace('\\', "/");
    let cwd = cwd.trim_end_matches('/');
    let base = cwd.split("/.claude/worktrees/").next().unwrap_or(cwd);
    let p = Path::new(base);
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.is_empty() {
        return "알 수 없음".to_string();
    }
    let lower = name.to_lowercase();
    let generic = GENERIC.contains(&lower.as_str()) || lower.starts_with("build") || lower.starts_with("scratch-");
    match p.parent().and_then(|x| x.file_name()).and_then(|s| s.to_str()) {
        // 윗 폴더가 뜻 없는 식별자(UUID)면 붙이지 않는다.
        Some(parent) if generic && !parent.is_empty() && !is_uuid(parent) => format!("{}/{}", parent, name),
        _ => name.to_string(),
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().enumerate().all(|(i, b)| if matches!(i, 8 | 13 | 18 | 23) { b == b'-' } else { b.is_ascii_hexdigit() })
}

/// 파일에서 지난번 이후 새로 붙은 완성된 줄만 읽어 상태에 반영한다.
fn update_file(path: &Path, kind: Kind, st: &mut FileState) {
    let Ok(mut f) = fs::File::open(path) else { return };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < st.offset {
        // 파일이 줄어들었으면(교체됨) 처음부터 다시 읽는다.
        *st = FileState::default();
    }
    if len == st.offset || f.seek(SeekFrom::Start(st.offset)).is_err() {
        return;
    }
    // 큰 기록 파일을 통째로 메모리에 올리지 않도록 한 줄씩 읽는다.
    let mut reader = BufReader::with_capacity(1 << 16, f);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        let Ok(n) = reader.read_until(b'\n', &mut raw) else { break };
        // 아직 다 쓰이지 않은 마지막 줄은 다음에 읽는다.
        if n == 0 || raw.last() != Some(&b'\n') {
            break;
        }
        st.offset += n as u64;
        let Ok(line) = std::str::from_utf8(&raw[..n - 1]) else { continue };
        match kind {
            Kind::Claude => claude_line(line, st),
            Kind::Codex => codex_line(line, st),
        }
    }
}

/// 모델 이름의 요금 줄을 찾고, 없으면 이름을 적어 둔다.
fn price_of(model: &str, unpriced: &mut BTreeSet<String>) -> Option<u16> {
    let found = pricing::index_of(model);
    if found.is_none() && !model.is_empty() && unpriced.len() < 20 {
        unpriced.insert(model.to_string());
    }
    found
}

fn claude_line(line: &str, st: &mut FileState) {
    if !line.contains("\"usage\"") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    let msg = &v["message"];
    let usage = &msg["usage"];
    if !usage.is_object() || msg["model"] == "<synthetic>" {
        return;
    }
    let Some(ts) = parse_ts(&v["timestamp"]) else { return };
    let price = price_of(msg["model"].as_str().unwrap_or(""), &mut st.unpriced);
    let key = msg["id"].as_str().map(|id| format!("{}:{}", id, v["requestId"].as_str().unwrap_or("")));
    st.events.push(Event {
        ts,
        tokens: Tokens {
            input: u(usage, "input_tokens"),
            cache_read: u(usage, "cache_read_input_tokens"),
            cache_write: u(usage, "cache_creation_input_tokens"),
            cache_write_1h: u(&usage["cache_creation"], "ephemeral_1h_input_tokens").min(u(usage, "cache_creation_input_tokens")),
            output: u(usage, "output_tokens"),
        },
        key,
        project: project_name(v["cwd"].as_str().unwrap_or("")),
        price,
    });
}

fn codex_line(line: &str, st: &mut FileState) {
    let is_meta = line.contains("\"session_meta\"");
    let is_turn = line.contains("\"turn_context\"");
    if !is_meta && !is_turn && !line.contains("\"token_count\"") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    let payload = &v["payload"];
    if v["type"] == "session_meta" {
        if let Some(cwd) = payload["cwd"].as_str() {
            st.project = project_name(cwd);
        }
        return;
    }
    if v["type"] == "turn_context" {
        if let Some(m) = payload["model"].as_str() {
            st.price = price_of(m, &mut st.unpriced);
        }
        return;
    }
    if payload["type"] != "token_count" {
        return;
    }
    let Some(ts) = parse_ts(&v["timestamp"]) else { return };

    let rl = &payload["rate_limits"];
    if rl.is_object() && st.limits.as_ref().map_or(true, |l| ts >= l.ts) {
        let (a, b) = (window(&rl["primary"]), window(&rl["secondary"]));
        if a.is_some() || b.is_some() {
            // 창 길이가 짧은 쪽을 primary로 맞춘다.
            let (primary, secondary) = match (a, b) {
                (Some(x), Some(y)) if x.window_minutes > y.window_minutes => (Some(y), Some(x)),
                (Some(x), None) if x.window_minutes > 24 * 60 => (None, Some(x)),
                other => other,
            };
            st.limits = Some(Limits { ts, primary, secondary, plan: rl["plan_type"].as_str().map(str::to_string) });
        }
    }

    // 누적값의 증가분만 더해 같은 이벤트가 반복 기록돼도 중복되지 않게 한다.
    let total = &payload["info"]["total_token_usage"];
    if total.is_object() {
        let now = (u(total, "input_tokens"), u(total, "cached_input_tokens"), u(total, "output_tokens"));
        let (din, dc, dout) = (now.0.saturating_sub(st.prev.0), now.1.saturating_sub(st.prev.1), now.2.saturating_sub(st.prev.2));
        st.prev = now;
        if din + dout > 0 {
            st.events.push(Event {
                ts,
                // Codex의 input_tokens는 캐시 읽기를 포함한다.
                tokens: Tokens { input: din.saturating_sub(dc), cache_read: dc, output: dout, ..Default::default() },
                key: None,
                project: if st.project.is_empty() { "알 수 없음".to_string() } else { st.project.clone() },
                price: st.price,
            });
        }
    }
}

struct Acc {
    /// 오늘의 시작
    start: i64,
    /// 기록을 보여주는 첫날의 시작
    hist_start: i64,
    now: i64,
    rhythm: Vec<u64>,
    projects: HashMap<String, ProjectUse>,
    days: Vec<DayUse>,
}

impl Acc {
    fn add(&mut self, e: &Event, kind: Kind, p: &mut Provider) {
        if e.ts < self.hist_start {
            return;
        }
        let total = e.tokens.total();
        let cost = e.price.map_or(0.0, |i| pricing::cost(i, &e.tokens));
        let di = (((e.ts - self.hist_start) / DAY) as usize).min(self.days.len() - 1);
        let d = &mut self.days[di];
        match kind {
            Kind::Claude => {
                d.claude += total;
                d.cost_claude += cost;
            }
            Kind::Codex => {
                d.codex += total;
                d.cost_codex += cost;
            }
        }
        if e.ts < self.start {
            return;
        }
        p.today.add(&e.tokens);
        p.cost_today += cost;
        let mins = (self.now - e.ts) / 60;
        if (0..RHYTHM_MINUTES as i64).contains(&mins) {
            self.rhythm[RHYTHM_MINUTES - 1 - mins as usize] += total;
        }
        let pj = self.projects.entry(e.project.clone()).or_insert_with(|| ProjectUse { name: e.project.clone(), ..Default::default() });
        pj.total += total;
        pj.cache += e.tokens.cache_read + e.tokens.cache_write;
        match kind {
            Kind::Claude => pj.claude += total,
            Kind::Codex => pj.codex += total,
        }
    }
}

/// 최근 3분 안에 기록이 붙은 세션 파일 수를 「작업 중」으로 본다.
const ACTIVE_SECONDS: u64 = 180;

fn collect_kind(kind: Kind, dir: PathBuf, acc: &mut Acc, states: &mut HashMap<PathBuf, FileState>, unpriced: &mut BTreeSet<String>) -> (Provider, u32) {
    let mut p = Provider { found: dir.is_dir(), ..Default::default() };
    if !p.found {
        return (p, 0);
    }
    let mut files = Vec::new();
    jsonl_files(&dir, &mut files, 0);
    files.sort_by(|a, b| b.1.cmp(&a.1));
    let recent = SystemTime::now() - std::time::Duration::from_secs(ACTIVE_SECONDS);
    let active = files.iter().filter(|(_, m)| *m >= recent).count() as u32;
    let start_sys = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(acc.hist_start.max(0) as u64);
    let mut seen: HashSet<&str> = HashSet::new();
    let mut latest: Option<&Limits> = None;

    // 최근 7일 안에 바뀐 파일 전부 + (Codex 한도 값을 찾기 위해) 가장 최근 파일 몇 개
    let wanted: Vec<&PathBuf> = files
        .iter()
        .enumerate()
        .filter(|(i, (_, m))| *m >= start_sys || (kind == Kind::Codex && *i < 5))
        .map(|(_, (path, _))| path)
        .collect();
    for path in &wanted {
        let st = states.entry((*path).clone()).or_default();
        update_file(path, kind, st);
        // 오래 켜 두어도 기억하는 양이 늘지 않도록 보여주는 기간보다 오래된 기록은 버린다.
        st.events.retain(|e| e.ts >= acc.hist_start);
    }
    for path in &wanted {
        let Some(st) = states.get(*path) else { continue };
        unpriced.extend(st.unpriced.iter().cloned());
        if let Some(l) = &st.limits {
            if latest.map_or(true, |x| l.ts > x.ts) {
                latest = Some(l);
            }
        }
        for e in &st.events {
            if let Some(k) = &e.key {
                if !seen.insert(k.as_str()) {
                    continue;
                }
            }
            acc.add(e, kind, &mut p);
        }
    }
    if let Some(l) = latest {
        p.primary = l.primary.clone();
        p.secondary = l.secondary.clone();
        p.plan = l.plan.clone();
        p.limits_observed_at = Some(l.ts);
    }
    (p, active)
}

/// 상태줄 연결이 저장한 Claude 한도 값(있을 때만).
/// 형식: {"observed_at":초,"five_hour":{"used_percentage":..,"resets_at":초},"seven_day":{...}}
fn read_claude_bridge(home: &Path, p: &mut Provider) {
    let bridge = home.join(".deskdusk").join("claude-limits.json");
    let Some(v) = fs::read_to_string(bridge).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { return };
    let w = |k: &str, mins: i64| {
        let o = &v[k];
        Some(LimitWindow {
            used_percent: o.get("used_percentage")?.as_f64()?,
            window_minutes: mins,
            resets_at: o.get("resets_at").and_then(Value::as_i64).unwrap_or(0),
            label: None,
        })
    };
    p.primary = w("five_hour", 300);
    p.secondary = w("seven_day", 10080);
    p.limits_observed_at = v["observed_at"].as_i64();
}

pub fn collect() -> Usage {
    let mut states = cache().lock().unwrap_or_else(|e| e.into_inner());
    collect_at(&home(), Local::now().timestamp(), midnight().timestamp(), &mut states)
}

/// 홈 폴더·지금 시각·오늘의 시작을 받아 모은다(시험에서 가짜 폴더와 시각을 넣을 수 있게 분리).
fn collect_at(home: &Path, now: i64, start: i64, states: &mut HashMap<PathBuf, FileState>) -> Usage {
    let hist_start = start - (HISTORY_DAYS as i64 - 1) * DAY;
    let days = (0..HISTORY_DAYS as i64).map(|i| DayUse { start: hist_start + i * DAY, ..Default::default() }).collect();
    let mut acc = Acc { start, hist_start, now, rhythm: vec![0; RHYTHM_MINUTES], projects: HashMap::new(), days };
    let mut unpriced = BTreeSet::new();

    let (codex, codex_active) = collect_kind(Kind::Codex, home.join(".codex").join("sessions"), &mut acc, states, &mut unpriced);
    let (mut claude, claude_active) = collect_kind(Kind::Claude, home.join(".claude").join("projects"), &mut acc, states, &mut unpriced);
    read_claude_bridge(home, &mut claude);
    // 선택 기능: 서버에 직접 물은 값이 있으면 그것을 쓴다(기본은 꺼짐).
    if let Some((limits, note)) = crate::claude_api::current(home, now) {
        claude.limits_source = Some("direct".into());
        claude.limits_note = note;
        if let Some(l) = limits {
            claude.primary = l.primary;
            claude.secondary = l.secondary;
            claude.more = l.more;
            claude.limits_observed_at = Some(l.observed_at);
        }
    }

    let mut projects: Vec<ProjectUse> = acc.projects.into_values().collect();
    projects.sort_by(|a, b| b.total.cmp(&a.total));
    projects.truncate(TOP_PROJECTS);

    Usage {
        generated_at: now,
        codex,
        claude,
        gemini: Provider { found: home.join(".gemini").is_dir(), ..Default::default() },
        grok: Provider { found: home.join(".grok").is_dir(), ..Default::default() },
        rhythm: acc.rhythm,
        agents: Agents { claude: claude_active, codex: codex_active },
        projects,
        daily: acc.days,
        unpriced: unpriced.into_iter().collect(),
        pricing_as_of: pricing::as_of().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    /// 시험마다 따로 쓰는 가짜 홈 폴더. 끝나면 지운다.
    struct Home(PathBuf);
    impl Home {
        fn new() -> Self {
            let d = std::env::temp_dir().join(format!("deskdusk-test-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(&d).unwrap();
            Home(d)
        }
        fn append(&self, rel: &str, text: &str) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut f = fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
            f.write_all(text.as_bytes()).unwrap();
        }
        fn write(&self, rel: &str, text: &str) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
    }
    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn iso(t: i64) -> String {
        DateTime::from_timestamp(t, 0).unwrap().to_rfc3339()
    }
    fn now() -> i64 {
        Local::now().timestamp()
    }
    fn claude(id: &str, t: i64, cwd: &str, tok: (u64, u64, u64, u64)) -> String {
        json!({"type":"assistant","timestamp":iso(t),"cwd":cwd,"requestId":format!("req-{id}"),
            "message":{"id":id,"model":"claude-test","usage":{"input_tokens":tok.0,"cache_read_input_tokens":tok.1,"cache_creation_input_tokens":tok.2,"output_tokens":tok.3}}})
        .to_string()
            + "\n"
    }
    fn codex_meta(cwd: &str) -> String {
        json!({"type":"session_meta","timestamp":iso(now()),"payload":{"cwd":cwd}}).to_string() + "\n"
    }
    fn codex(t: i64, total: (u64, u64, u64), rl: Value) -> String {
        json!({"type":"event_msg","timestamp":iso(t),"payload":{"type":"token_count",
            "info":{"total_token_usage":{"input_tokens":total.0,"cached_input_tokens":total.1,"output_tokens":total.2}},"rate_limits":rl}})
        .to_string()
            + "\n"
    }
    fn run(h: &Home, states: &mut HashMap<PathBuf, FileState>) -> Usage {
        let n = now();
        collect_at(&h.0, n, n - 36_000, states)
    }
    const C1: &str = ".claude/projects/p1/a.jsonl";
    const C2: &str = ".claude/projects/p1/b.jsonl";
    const X1: &str = ".codex/sessions/2026/11/10/rollout-a.jsonl";

    #[test]
    fn 프로젝트_이름_규칙() {
        assert_eq!(project_name("/Users/a/sketchPad"), "sketchPad");
        assert_eq!(project_name("/Users/a/RunLog/build46"), "RunLog/build46");
        assert_eq!(project_name("/Users/a/YogaFlow/mobile"), "YogaFlow/mobile");
        assert_eq!(project_name("/Users/a/YogaFlow/Mobile"), "YogaFlow/Mobile");
        assert_eq!(project_name("/Users/a/sketchPad/.claude/worktrees/fix-1"), "sketchPad");
        assert_eq!(project_name("/Users/a/sketchPad/.claude/worktrees/fix-1/ios"), "sketchPad");
        assert_eq!(project_name("/x/scratch-2026-11-10-45c921"), "x/scratch-2026-11-10-45c921");
        assert_eq!(project_name("/w/93f1a3d4-ed87-42ef-8851-ee5ffe5e23dc/scratch-2026-11-05-157e0c"), "scratch-2026-11-05-157e0c");
        assert_eq!(project_name("/w/DeskDusk/src-tauri"), "DeskDusk/src-tauri");
        assert_eq!(project_name("/Users/a/builder"), "a/builder");
        assert_eq!(project_name(""), "알 수 없음");
        assert_eq!(project_name("/"), "알 수 없음");
        assert_eq!(project_name("/src"), "src");
    }

    #[test]
    fn 자료가_없으면_빈_값() {
        let h = Home::new();
        let u = run(&h, &mut HashMap::new());
        assert!(!u.claude.found && !u.codex.found && !u.gemini.found && !u.grok.found);
        assert_eq!(u.claude.today.total() + u.codex.today.total(), 0);
        assert!(u.projects.is_empty());
        assert_eq!(u.rhythm.len(), 60);
        assert!(u.claude.primary.is_none() && u.codex.secondary.is_none());
    }

    #[test]
    fn claude_합계와_중복_제거() {
        let h = Home::new();
        let t = now() - 600;
        // 같은 응답이 한 파일에 두 번, 다른 파일(이어받은 세션)에 한 번 더 적힌 경우
        h.append(C1, &claude("m1", t, "/w/sketchPad", (10, 100, 20, 5)));
        h.append(C1, &claude("m1", t, "/w/sketchPad", (10, 100, 20, 5)));
        h.append(C2, &claude("m1", t, "/w/sketchPad", (10, 100, 20, 5)));
        h.append(C2, &claude("m2", t, "/w/YogaFlow/mobile", (1, 2, 3, 4)));
        let u = run(&h, &mut HashMap::new());
        assert!(u.claude.found);
        let d = &u.claude.today;
        assert_eq!((d.input, d.cache_read, d.cache_write, d.output), (11, 102, 23, 9));
        assert_eq!(u.projects.len(), 2);
        assert_eq!((u.projects[0].name.as_str(), u.projects[0].total, u.projects[0].cache, u.projects[0].claude, u.projects[0].codex), ("sketchPad", 135, 120, 135, 0));
        assert_eq!((u.projects[1].name.as_str(), u.projects[1].total), ("YogaFlow/mobile", 10));
    }

    #[test]
    fn claude_제외_대상() {
        let h = Home::new();
        let n = now();
        h.append(C1, &claude("old", n - 40_000, "/w/a", (1000, 0, 0, 0))); // 오늘 시작 이전
        h.append(C1, &json!({"timestamp":iso(n),"cwd":"/w/a","message":{"id":"s","model":"<synthetic>","usage":{"input_tokens":500,"output_tokens":500}}}).to_string());
        h.append(C1, "\n이건 JSON이 아닌 줄 \"usage\"\n{\"usage\": 깨진 줄\n");
        h.append(C1, &json!({"timestamp":"시각 아님","message":{"id":"x","usage":{"input_tokens":7}}}).to_string());
        h.append(C1, "\n");
        h.append(C1, &claude("ok", n - 5, "/w/a", (3, 0, 0, 4)));
        h.write(".claude/projects/p1/notes.txt", &claude("txt", n, "/w/a", (999, 0, 0, 0)));
        let u = run(&h, &mut HashMap::new());
        assert_eq!(u.claude.today.total(), 7);
    }

    #[test]
    fn 쓰다_만_마지막_줄은_다음에_읽는다() {
        let h = Home::new();
        let t = now() - 60;
        let mut states = HashMap::new();
        let line = claude("m1", t, "/w/a", (10, 0, 0, 1));
        let (head, tail) = line.split_at(line.len() / 2);
        h.append(C1, &claude("m0", t, "/w/a", (1, 0, 0, 1)));
        h.append(C1, head);
        assert_eq!(run(&h, &mut states).claude.today.total(), 2);
        assert_eq!(run(&h, &mut states).claude.today.total(), 2); // 변화 없으면 그대로
        h.append(C1, tail);
        assert_eq!(run(&h, &mut states).claude.today.total(), 13);
        h.append(C1, &claude("m2", t, "/w/a", (100, 0, 0, 0)));
        assert_eq!(run(&h, &mut states).claude.today.total(), 113); // 새로 붙은 줄만 더해진다
    }

    #[test]
    fn 파일이_교체되면_처음부터_다시_읽는다() {
        let h = Home::new();
        let t = now() - 60;
        let mut states = HashMap::new();
        h.append(C1, &claude("m1", t, "/w/a", (50, 0, 0, 0)));
        h.append(C1, &claude("m2", t, "/w/a", (50, 0, 0, 0)));
        assert_eq!(run(&h, &mut states).claude.today.total(), 100);
        h.write(C1, &claude("m9", t, "/w/a", (7, 0, 0, 0)));
        assert_eq!(run(&h, &mut states).claude.today.total(), 7);
    }

    #[test]
    fn codex_누적값은_증가분만_더한다() {
        let h = Home::new();
        let t = now() - 300;
        h.append(X1, &codex_meta("/w/RunLog/build46"));
        h.append(X1, &codex(t, (1000, 800, 50), Value::Null));
        h.append(X1, &codex(t + 1, (1000, 800, 50), Value::Null)); // 같은 값 반복 기록
        h.append(X1, &codex(t + 2, (1500, 1100, 80), Value::Null));
        let u = run(&h, &mut HashMap::new());
        let d = &u.codex.today;
        // 입력 1500 중 캐시 1100 → 새 입력 400, 출력 80
        assert_eq!((d.input, d.cache_read, d.cache_write, d.output), (400, 1100, 0, 80));
        assert_eq!((u.projects[0].name.as_str(), u.projects[0].codex, u.projects[0].claude), ("RunLog/build46", 1580, 0));
    }

    #[test]
    fn codex_어제_시작한_세션은_오늘_증가분만() {
        let h = Home::new();
        let n = now();
        h.append(X1, &codex(n - 50_000, (9000, 0, 1000), Value::Null)); // 오늘 시작 이전
        h.append(X1, &codex(n - 100, (9500, 300, 1200), Value::Null));
        let u = run(&h, &mut HashMap::new());
        assert_eq!(u.codex.today.total(), 700);
        assert_eq!(u.projects[0].name, "알 수 없음");
    }

    #[test]
    fn codex_한도는_가장_최근_값과_짧은_창이_먼저() {
        let h = Home::new();
        let n = now();
        let rl = |a: f64, b: f64| json!({"plan_type":"pro","primary":{"used_percent":a,"window_minutes":10080,"resets_at":n+9000},"secondary":{"used_percent":b,"window_minutes":300,"resets_at":n+600}});
        h.append(X1, &codex(n - 500, (10, 0, 1), rl(20.0, 30.0)));
        h.append(X1, &codex(n - 100, (20, 0, 2), rl(25.0, 35.0)));
        h.append(".codex/sessions/2026/11/09/rollout-b.jsonl", &codex(n - 300, (5, 0, 1), rl(1.0, 2.0)));
        let u = run(&h, &mut HashMap::new());
        let (p, s) = (u.codex.primary.unwrap(), u.codex.secondary.unwrap());
        assert_eq!((p.window_minutes, p.used_percent, p.resets_at), (300, 35.0, n + 600));
        assert_eq!((s.window_minutes, s.used_percent), (10080, 25.0));
        assert_eq!(u.codex.plan.as_deref(), Some("pro"));
        assert_eq!(u.codex.limits_observed_at, Some(n - 100));
    }

    #[test]
    fn codex_긴_창_하나뿐이면_둘째_칸에() {
        let h = Home::new();
        let n = now();
        h.append(X1, &codex(n - 10, (10, 0, 1), json!({"plan_type":"free","primary":{"used_percent":98.0,"window_minutes":43200,"resets_at":n+100},"secondary":null})));
        let u = run(&h, &mut HashMap::new());
        assert!(u.codex.primary.is_none());
        assert_eq!(u.codex.secondary.unwrap().window_minutes, 43200);
    }

    #[test]
    fn claude_한도는_상태줄_파일에서() {
        let h = Home::new();
        fs::create_dir_all(h.0.join(".claude/projects")).unwrap();
        let mut states = HashMap::new();
        assert!(run(&h, &mut states).claude.primary.is_none());
        h.write(".deskdusk/claude-limits.json", r#"{"observed_at":111,"five_hour":{"used_percentage":82,"resets_at":222},"seven_day":{"used_percentage":14.5,"resets_at":333}}"#);
        let u = run(&h, &mut states);
        let (p, s) = (u.claude.primary.unwrap(), u.claude.secondary.unwrap());
        assert_eq!((p.used_percent, p.window_minutes, p.resets_at), (82.0, 300, 222));
        assert_eq!((s.used_percent, s.window_minutes, s.resets_at), (14.5, 10080, 333));
        assert_eq!(u.claude.limits_observed_at, Some(111));
        // 5시간 값이 비어 있거나 파일이 깨져도 멈추지 않는다
        h.write(".deskdusk/claude-limits.json", r#"{"observed_at":111,"five_hour":null,"seven_day":{"used_percentage":14,"resets_at":333}}"#);
        let u = run(&h, &mut states);
        assert!(u.claude.primary.is_none() && u.claude.secondary.is_some());
        h.write(".deskdusk/claude-limits.json", "{깨진 파일");
        assert!(run(&h, &mut states).claude.secondary.is_none());
    }

    #[test]
    fn 리듬은_분_단위_칸에_들어간다() {
        let h = Home::new();
        let n = now();
        h.append(C1, &claude("a", n - 10, "/w/a", (5, 0, 0, 0)));
        h.append(C1, &claude("b", n - 30 * 60 - 10, "/w/a", (7, 0, 0, 0)));
        h.append(C1, &claude("c", n - 2 * 3600, "/w/a", (100, 0, 0, 0))); // 1시간보다 오래됨
        let u = collect_at(&h.0, n, n - 36_000, &mut HashMap::new());
        assert_eq!(u.rhythm[59], 5);
        assert_eq!(u.rhythm[29], 7);
        assert_eq!(u.rhythm.iter().sum::<u64>(), 12);
        assert_eq!(u.claude.today.total(), 112);
    }

    #[test]
    fn 프로젝트는_많이_태운_순서와_서비스별_합계() {
        let h = Home::new();
        let t = now() - 60;
        h.append(C1, &claude("a", t, "/w/alpha", (100, 0, 0, 0)));
        h.append(C1, &claude("b", t, "/w/beta", (300, 0, 0, 0)));
        h.append(X1, &codex_meta("/w/alpha"));
        h.append(X1, &codex(t, (500, 100, 0), Value::Null));
        let u = run(&h, &mut HashMap::new());
        let names: Vec<_> = u.projects.iter().map(|p| (p.name.as_str(), p.total, p.claude, p.codex)).collect();
        assert_eq!(names, vec![("alpha", 600, 100, 500), ("beta", 300, 300, 0)]);
    }

    #[test]
    fn 작업_중_세션은_방금_기록된_파일_수() {
        let h = Home::new();
        h.append(C1, &claude("a", now(), "/w/a", (1, 0, 0, 0)));
        h.append(C2, &claude("b", now(), "/w/a", (1, 0, 0, 0)));
        h.append(X1, &codex(now(), (1, 0, 1), Value::Null));
        let u = run(&h, &mut HashMap::new());
        assert_eq!((u.agents.claude, u.agents.codex), (2, 1));
    }

    #[test]
    fn 최근_7일은_날짜별로_나뉘고_오늘_합계는_오늘만() {
        let h = Home::new();
        let (start, now) = (1_000_000 - 1_000_000 % DAY, 1_000_000 - 1_000_000 % DAY + 3600);
        // 오늘, 어제, 6일 전, 7일 전(범위 밖)
        h.append(C1, &claude("d0", now - 60, "/w/a", (1, 0, 0, 1)));
        h.append(C1, &claude("d1", start - 100, "/w/a", (10, 0, 0, 10)));
        h.append(C1, &claude("d6", start - 6 * DAY + 5, "/w/a", (100, 0, 0, 100)));
        h.append(C1, &claude("d7", start - 6 * DAY - 5, "/w/a", (1000, 0, 0, 1000)));
        let mut states = HashMap::new();
        let u = collect_at(&h.0, now, start, &mut states);
        assert_eq!(u.daily.len(), HISTORY_DAYS);
        assert_eq!(u.daily.iter().map(|d| d.claude).collect::<Vec<_>>(), vec![200, 0, 0, 0, 0, 20, 2]);
        assert_eq!(u.daily[6].start, start);
        assert_eq!(u.daily[0].start, start - 6 * DAY);
        assert_eq!(u.claude.today.total(), 2);
        // 다시 읽어도 같은 값(이어 읽기에서 중복되지 않는다)
        let u2 = collect_at(&h.0, now, start, &mut states);
        assert_eq!(u2.daily.iter().map(|d| d.claude).sum::<u64>(), 222);
    }

    #[test]
    fn 추정_비용은_요금표에_있는_모델만_더하고_없는_모델은_이름을_알린다() {
        let h = Home::new();
        let (start, now) = (2_000_000 - 2_000_000 % DAY, 2_000_000 - 2_000_000 % DAY + 3600);
        let line = |id: &str, model: &str| json!({"timestamp":iso(now - 30),"cwd":"/w/a","requestId":"r",
            "message":{"id":id,"model":model,"usage":{"input_tokens":1_000_000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"output_tokens":1_000_000}}}).to_string() + "\n";
        h.append(C1, &line("p1", "claude-opus-5-5"));
        h.append(C1, &line("p2", "아직-없는-모델"));
        let mut states = HashMap::new();
        let u = collect_at(&h.0, now, start, &mut states);
        // 입력 4달러 + 출력 20달러. 요금표에 없는 모델은 0으로 친다.
        assert!((u.claude.cost_today - 24.0).abs() < 1e-9);
        assert!((u.daily[6].cost_claude - 24.0).abs() < 1e-9);
        assert_eq!(u.unpriced, vec!["아직-없는-모델".to_string()]);
        assert_eq!(u.claude.today.total(), 4_000_000);
        assert!(!u.pricing_as_of.is_empty());
    }

    #[test]
    fn 한_시간_캐시_쓰기는_더_비싼_요금으로_계산한다() {
        let h = Home::new();
        let (start, now) = (3_000_000 - 3_000_000 % DAY, 3_000_000 - 3_000_000 % DAY + 3600);
        h.append(C1, &(json!({"timestamp":iso(now - 30),"cwd":"/w/a","requestId":"r",
            "message":{"id":"w1","model":"claude-opus-5-5","usage":{"input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0,
                "cache_creation_input_tokens":3_000_000,"cache_creation":{"ephemeral_5m_input_tokens":1_000_000,"ephemeral_1h_input_tokens":2_000_000}}}}).to_string() + "\n"));
        let mut states = HashMap::new();
        let u = collect_at(&h.0, now, start, &mut states);
        assert_eq!((u.claude.today.cache_write, u.claude.today.cache_write_1h), (3_000_000, 2_000_000));
        // 5분 캐시 100만 × 5달러 + 1시간 캐시 200만 × 8달러
        assert!((u.claude.cost_today - 21.0).abs() < 1e-9);
    }

    #[test]
    fn 윈도우_경로도_프로젝트_이름을_뽑는다() {
        assert_eq!(project_name(r"C:\Users\a\sketchPad"), "sketchPad");
        assert_eq!(project_name(r"C:\Users\a\YogaFlow\mobile"), "YogaFlow/mobile");
        assert_eq!(project_name(r"C:\Users\a\sketchPad\.claude\worktrees\fix-1"), "sketchPad");
        assert_eq!(project_name("/Users/a/sketchPad/"), "sketchPad");
    }
}
