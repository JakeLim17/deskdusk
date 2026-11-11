//! 선택 기능: 정확한 Claude 한도 가져오기. 기본은 꺼짐이다.
//! 켜면 이 컴퓨터에 저장된 Claude Code 로그인 정보(맥은 키체인, 그 밖은 ~/.claude/.credentials.json)로
//! Anthropic 서버에 한도 사용률을 묻는다. Claude Code의 /usage 화면이 쓰는 것과 같은 값이다.
//! - 로그인 정보는 요청을 보낼 때만 읽고, 저장하거나 기록에 남기지 않는다. 고치지도 않는다(만료되면 그만둔다).
//! - 공식으로 문서화된 경로가 아니라 언제든 바뀌거나 막힐 수 있다. 자주 물으면 한동안 막히므로 10분에 한 번만 묻는다.

use crate::usage::{self, LimitWindow};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

const URL: &str = "https://api.anthropic.com/api/oauth/usage";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const INTERVAL: i64 = 600;
const BACKOFF: i64 = 1800;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Limits {
    pub observed_at: i64,
    pub primary: Option<LimitWindow>,
    pub secondary: Option<LimitWindow>,
    /// 모델별 주간 한도 등 그 밖의 창
    pub more: Vec<LimitWindow>,
}

fn flag_path(home: &Path) -> PathBuf {
    home.join(".deskdusk").join("claude-direct.json")
}

pub fn is_on_at(home: &Path) -> bool {
    std::fs::read_to_string(flag_path(home)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).is_some_and(|v| v["on"] == true)
}

pub fn set_on_at(home: &Path, on: bool) -> Result<(), String> {
    let p = flag_path(home);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, if on { r#"{"on":true}"# } else { r#"{"on":false}"# }).map_err(|e| e.to_string())
}

pub fn is_on() -> bool {
    is_on_at(&usage::home())
}

pub fn set_on(on: bool) -> Result<bool, String> {
    set_on_at(&usage::home(), on)?;
    // 켜고 끌 때는 기억해 둔 결과를 버려 바로 다시 묻게 한다.
    *state().lock().unwrap_or_else(|e| e.into_inner()) = State::default();
    Ok(on)
}

/// 응답의 시각 값(ISO 문자열 또는 초·밀리초 숫자)을 유닉스 초로.
fn to_seconds(v: &Value) -> i64 {
    if let Some(s) = v.as_str() {
        return chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp()).unwrap_or(0);
    }
    match v.as_f64() {
        Some(n) if n > 1e12 => (n / 1000.0) as i64,
        Some(n) if n > 0.0 => n as i64,
        _ => 0,
    }
}

fn window(v: &Value, minutes: i64, label: Option<String>) -> Option<LimitWindow> {
    let used = v.get("utilization")?.as_f64()?;
    Some(LimitWindow { used_percent: used.clamp(0.0, 100.0), window_minutes: minutes, resets_at: to_seconds(&v["resets_at"]), label, ..Default::default() })
}

/// 응답을 한도 창으로 바꾼다. 형식이 문서화돼 있지 않아, 「utilization」이 든 항목만 골라 조심스럽게 읽는다.
pub fn parse(v: &Value, now: i64) -> Option<Limits> {
    let o = v.as_object()?;
    let mut out = Limits { observed_at: now, ..Default::default() };
    for (k, item) in o {
        if !item.is_object() {
            continue;
        }
        match k.as_str() {
            "five_hour" => out.primary = window(item, 300, None),
            "seven_day" => out.secondary = window(item, 10080, None),
            _ => {
                // seven_day_opus → 「Opus」처럼 모델별 주간 한도
                if let Some(name) = k.strip_prefix("seven_day_") {
                    let mut label = name.replace('_', " ");
                    if let Some(first) = label.get(0..1) {
                        label = first.to_uppercase() + &label[1..];
                    }
                    if let Some(w) = window(item, 10080, Some(label)) {
                        out.more.push(w);
                    }
                }
            }
        }
    }
    // 새 형식: limits 목록(session, weekly_all, weekly_scoped). 있으면 이쪽이 화면과 같은 값이라 우선한다.
    if let Some(list) = v["limits"].as_array() {
        let mut more = Vec::new();
        for item in list {
            let Some(pct) = item["percent"].as_f64() else { continue };
            let win = |minutes: i64, label: Option<String>| LimitWindow { used_percent: pct.clamp(0.0, 100.0), window_minutes: minutes, resets_at: to_seconds(&item["resets_at"]), label, ..Default::default() };
            match item["kind"].as_str() {
                Some("session") => out.primary = Some(win(300, None)),
                Some("weekly_all") => out.secondary = Some(win(10080, None)),
                Some("weekly_scoped") => {
                    // 모델별(예: Fable)이나 사용처별 주간 한도
                    let scope = &item["scope"];
                    let name = scope["model"]["display_name"].as_str().or_else(|| scope["surface"]["display_name"].as_str()).or_else(|| scope["surface"].as_str());
                    if let Some(name) = name {
                        more.push(win(10080, Some(name.chars().take(24).collect())));
                    }
                }
                _ => {}
            }
        }
        if !more.is_empty() {
            out.more = more;
        }
    }
    (out.primary.is_some() || out.secondary.is_some() || !out.more.is_empty()).then_some(out)
}

/// 로그인 정보에서 접근 토큰과 만료 시각(초)을 꺼낸다. 값은 돌려주기만 하고 어디에도 남기지 않는다.
fn token_from(json: &str) -> Option<(String, i64)> {
    let v: Value = serde_json::from_str(json.trim()).ok()?;
    let o = &v["claudeAiOauth"];
    let token = o["accessToken"].as_str()?.to_string();
    Some((token, to_seconds(&o["expiresAt"])))
}

fn read_credentials(home: &Path) -> Result<String, String> {
    if cfg!(target_os = "macos") {
        let out = Command::new("/usr/bin/security").args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"]).stderr(Stdio::null()).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            return String::from_utf8(out.stdout).map_err(|_| "로그인 정보를 읽지 못했어요.".to_string());
        }
    }
    std::fs::read_to_string(home.join(".claude").join(".credentials.json")).map_err(|_| "Claude Code 로그인 정보를 찾지 못했어요. 터미널에서 Claude Code에 로그인해 주세요.".to_string())
}

/// 서버에 묻는다. 토큰이 명령줄에 드러나지 않도록 curl 설정을 표준 입력으로 넘긴다.
fn request(token: &str) -> Result<(u16, String), String> {
    if token.contains('"') || token.contains('\n') {
        return Err("로그인 정보 형식이 올바르지 않아요.".into());
    }
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", "12", "-w", "\n%{http_code}", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let config = format!("url = \"{URL}\"\nheader = \"Authorization: Bearer {token}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\nheader = \"Accept: application/json\"\nuser-agent = \"DeskDusk/{}\"\n", env!("CARGO_PKG_VERSION"));
    child.stdin.take().ok_or("요청을 보내지 못했어요.")?.write_all(config.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), "0"));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

fn fetch(home: &Path, now: i64) -> Result<Limits, (String, i64)> {
    let soon = |m: &str| (m.to_string(), INTERVAL);
    let (token, expires) = token_from(&read_credentials(home).map_err(|e| (e, INTERVAL))?).ok_or_else(|| soon("Claude Code 로그인 정보를 읽지 못했어요."))?;
    if expires > 0 && expires <= now {
        return Err(soon("로그인이 만료됐어요. Claude Code를 한 번 실행하면 다시 받아 와요."));
    }
    let (code, body) = request(&token).map_err(|e| (e, INTERVAL))?;
    match code {
        200 => serde_json::from_str::<Value>(&body).ok().and_then(|v| parse(&v, now)).ok_or_else(|| ("서버 응답 형식이 바뀐 것 같아요.".to_string(), BACKOFF)),
        401 | 403 => Err(soon("로그인이 만료됐거나 권한이 없어요. Claude Code를 한 번 실행해 주세요.")),
        429 => Err(("서버가 잠시 요청을 막았어요. 30분 뒤 다시 물어요.".to_string(), BACKOFF)),
        0 => Err(soon("서버에 연결하지 못했어요.")),
        n => Err((format!("서버가 오류를 돌려줬어요({n})."), BACKOFF)),
    }
}

/// 점검용: 서버 응답의 모양(항목 이름과 숫자)을 글자로 돌려준다. 로그인 정보는 포함하지 않는다.
pub fn debug_shape() -> String {
    let home = usage::home();
    let now = chrono::Local::now().timestamp();
    let Ok(cred) = read_credentials(&home) else { return "로그인 정보 없음".into() };
    let Some((token, expires)) = token_from(&cred) else { return "로그인 정보 형식 오류".into() };
    if expires > 0 && expires <= now {
        return "로그인 만료".into();
    }
    match request(&token) {
        Ok((code, body)) => match serde_json::from_str::<Value>(&body) {
            Ok(v) => format!("상태 {code}\n{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
            Err(_) => format!("상태 {code}, 본문 {}바이트(JSON 아님)", body.len()),
        },
        Err(e) => e,
    }
}

#[derive(Default)]
struct State {
    next_at: i64,
    last: Option<Limits>,
    note: Option<String>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State::default()))
}

/// 켜져 있으면 (마지막으로 받은 값, 안내 문구)를 돌려준다. 10분에 한 번만 실제로 묻는다.
pub fn current(home: &Path, now: i64) -> Option<(Option<Limits>, Option<String>)> {
    if !is_on_at(home) {
        return None;
    }
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
    if now >= s.next_at {
        match fetch(home, now) {
            Ok(l) => {
                s.last = Some(l);
                s.note = None;
                s.next_at = now + INTERVAL;
            }
            Err((msg, wait)) => {
                s.note = Some(msg);
                s.next_at = now + wait;
            }
        }
    }
    Some((s.last.clone(), s.note.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 응답에서_세션_주간_모델별_한도를_읽는다() {
        let v = json!({
            "five_hour": {"utilization": 7.0, "resets_at": "2026-11-10T22:30:00+00:00"},
            "seven_day": {"utilization": 41.0, "resets_at": "2026-11-17T02:00:00Z"},
            "seven_day_fable": {"utilization": 46.2, "resets_at": "2026-11-17T02:00:00Z"},
            "seven_day_opus": null,
            "extra_usage": {"is_enabled": false},
            "something_else": 3
        });
        let l = parse(&v, 100).unwrap();
        assert_eq!(l.observed_at, 100);
        let p = l.primary.unwrap();
        assert_eq!((p.used_percent, p.window_minutes), (7.0, 300));
        assert_eq!(p.resets_at, chrono::DateTime::parse_from_rfc3339("2026-11-10T22:30:00+00:00").unwrap().timestamp());
        assert_eq!(l.secondary.unwrap().used_percent, 41.0);
        assert_eq!(l.more.len(), 1);
        assert_eq!((l.more[0].label.as_deref(), l.more[0].used_percent, l.more[0].window_minutes), (Some("Fable"), 46.2, 10080));
    }

    #[test]
    fn 한도가_하나도_없거나_형식이_다르면_읽지_않는다() {
        assert_eq!(parse(&json!({"error": {"type": "x"}}), 1), None);
        assert_eq!(parse(&json!([1, 2]), 1), None);
        assert_eq!(parse(&json!({"five_hour": {"utilization": "많음"}}), 1), None);
        // 범위를 벗어난 값은 0~100으로 맞춘다.
        assert_eq!(parse(&json!({"seven_day": {"utilization": 130}}), 1).unwrap().secondary.unwrap().used_percent, 100.0);
    }

    #[test]
    fn 로그인_정보에서_토큰과_만료_시각만_꺼낸다() {
        let (t, e) = token_from(r#"{"claudeAiOauth":{"accessToken":"시험용-토큰","refreshToken":"x","expiresAt":1794000000000}}"#).unwrap();
        assert_eq!((t.as_str(), e), ("시험용-토큰", 1_794_000_000));
        assert!(token_from("{}").is_none());
        assert!(token_from("깨짐").is_none());
    }

    #[test]
    fn 꺼져_있으면_아무것도_하지_않는다() {
        let d = std::env::temp_dir().join(format!("deskdusk-direct-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(!is_on_at(&d));
        assert!(current(&d, 1).is_none());
        set_on_at(&d, true).unwrap();
        assert!(is_on_at(&d));
        set_on_at(&d, false).unwrap();
        assert!(!is_on_at(&d));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn limits_목록이_있으면_그것으로_세션_주간_모델별_한도를_읽는다() {
        let v = json!({
            "five_hour": {"utilization": 13.0, "resets_at": "2026-11-10T22:29:59.734363+00:00"},
            "seven_day": {"utilization": 42.0, "resets_at": "2026-11-17T01:59:59.734384+00:00"},
            "seven_day_opus": null,
            "limits": [
                {"kind": "session", "group": "session", "percent": 13, "resets_at": "2026-11-10T22:29:59.737402+00:00", "scope": null},
                {"kind": "weekly_all", "group": "weekly", "percent": 42, "resets_at": "2026-11-17T01:59:59.737420+00:00", "scope": null},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 47, "resets_at": "2026-11-17T01:59:59.737573+00:00",
                 "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null}},
                {"kind": "새로_생긴_종류", "percent": 5}
            ]
        });
        let l = parse(&v, 7).unwrap();
        assert_eq!(l.primary.as_ref().unwrap().used_percent, 13.0);
        assert_eq!(l.secondary.as_ref().unwrap().used_percent, 42.0);
        assert_eq!(l.more.len(), 1);
        assert_eq!((l.more[0].label.as_deref(), l.more[0].used_percent), (Some("Fable"), 47.0));
        assert!(l.more[0].resets_at > 0);
    }
}
