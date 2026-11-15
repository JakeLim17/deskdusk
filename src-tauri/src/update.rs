//! 새 판 알림: 깃허브의 최신 릴리스를 하루 한 번 확인해, 지금 판보다 새 판이 있으면 알린다.
//! 요청에는 판 번호가 든 User-Agent만 실린다(사용자 정보·사용량은 보내지 않는다). 설정에서 끌 수 있다.
//! 상태는 ~/.deskdusk/update.json 에 둔다: {"off": bool, "checked_at": 초, "latest": "0.1.3", "url": "..."}.

use crate::usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const API: &str = "https://api.github.com/repos/JakeLim17/deskdusk/releases/latest";
/// 알림에서 여는 주소는 이 앞부분으로 시작해야 한다(응답이 이상해도 다른 곳을 열지 않게).
pub const RELEASES: &str = "https://github.com/JakeLim17/deskdusk/releases/";
const INTERVAL: i64 = 24 * 3600;
/// 실패하면 이만큼 뒤에 다시 묻는다.
const RETRY: i64 = 3 * 3600;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct State {
    #[serde(default)]
    pub off: bool,
    #[serde(default)]
    pub checked_at: i64,
    #[serde(default)]
    pub latest: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// 화면에 넘기는 값: 새 판이 있으면 판 번호와 주소.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Info {
    pub on: bool,
    pub current: String,
    pub newer: Option<String>,
    pub url: Option<String>,
}

fn path(home: &Path) -> PathBuf {
    home.join(".deskdusk").join("update.json")
}

pub fn load_at(home: &Path) -> State {
    std::fs::read_to_string(path(home)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_at(home: &Path, s: &State) -> Result<(), String> {
    let p = path(home);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// "v0.1.12" 또는 "0.1.12" → (0, 1, 12). 숫자 세 개가 아니면 None.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim().trim_start_matches('v').split('.').map(|x| x.parse::<u32>().ok());
    let v = (it.next()??, it.next()??, it.next()??);
    if it.next().is_some() {
        return None;
    }
    Some(v)
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// 릴리스 응답에서 판 번호와 주소를 꺼낸다. 초안·미리 보기 판이거나 주소가 이 저장소가 아니면 None.
pub fn parse_release(v: &Value) -> Option<(String, String)> {
    if v["draft"] == true || v["prerelease"] == true {
        return None;
    }
    let tag = v["tag_name"].as_str()?;
    let (a, b, c) = parse_version(tag)?;
    let url = v["html_url"].as_str()?;
    if !url.starts_with(RELEASES) {
        return None;
    }
    Some((format!("{a}.{b}.{c}"), url.to_string()))
}

pub fn info_of(s: &State, current: &str) -> Info {
    let newer = s.latest.clone().filter(|l| !s.off && is_newer(l, current));
    Info { on: !s.off, current: current.to_string(), url: newer.as_ref().and(s.url.clone()), newer }
}

/// 물어볼 때가 됐는지: 꺼져 있지 않고, 마지막 확인에서 하루(실패했으면 3시간)가 지났을 때.
pub fn due(s: &State, now: i64) -> bool {
    if s.off {
        return false;
    }
    let wait = if s.latest.is_some() { INTERVAL } else { RETRY };
    now - s.checked_at >= wait || s.checked_at > now
}

fn request() -> Result<(u16, String), String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", "12", "-w", "\n%{http_code}", "-H", "Accept: application/vnd.github+json", "-A", &format!("DeskDusk/{}", env!("CARGO_PKG_VERSION")), API])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), "0"));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

/// 때가 됐으면 깃허브에 묻고 결과를 저장한다. 지금 상태를 돌려준다.
pub fn check_at(home: &Path, now: i64) -> State {
    let mut s = load_at(home);
    if !due(&s, now) {
        return s;
    }
    s.checked_at = now;
    match request() {
        Ok((200, body)) => {
            if let Some((latest, url)) = serde_json::from_str::<Value>(&body).ok().as_ref().and_then(parse_release) {
                s.latest = Some(latest);
                s.url = Some(url);
            } else {
                // 응답 모양이 다르면 지난 값은 지우고 3시간 뒤에 다시 묻는다.
                s.latest = None;
                s.url = None;
            }
        }
        _ => {
            s.latest = None;
            s.url = None;
        }
    }
    let _ = save_at(home, &s);
    s
}

pub fn current() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn info() -> Info {
    info_of(&load_at(&usage::home()), &current())
}

pub fn set_on(on: bool) -> Result<Info, String> {
    let home = usage::home();
    let mut s = load_at(&home);
    s.off = !on;
    if on {
        // 다시 켜면 바로 한 번 묻도록 확인 시각을 비운다.
        s.checked_at = 0;
    }
    save_at(&home, &s)?;
    Ok(info_of(&s, &current()))
}

/// 릴리스 페이지를 기본 브라우저로 연다. 이 저장소의 릴리스 주소만 연다.
pub fn open_release(url: &str) -> Result<(), String> {
    if !url.starts_with(RELEASES) || url.chars().any(|c| c.is_whitespace() || c == '"' || c == '&' || c == '^' || c == '|') {
        return Err("열 수 없는 주소예요.".into());
    }
    #[cfg(target_os = "macos")]
    let r = Command::new("/usr/bin/open").arg(url).status();
    #[cfg(target_os = "windows")]
    let r = {
        use std::os::windows::process::CommandExt;
        Command::new("explorer").arg(url).creation_flags(0x0800_0000).status()
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = Command::new("xdg-open").arg(url).status();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 판_번호를_읽고_비교한다() {
        assert_eq!(parse_version("v0.1.12"), Some((0, 1, 12)));
        assert_eq!(parse_version("0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version("v0.1"), None);
        assert_eq!(parse_version("v0.1.2-beta"), None);
        assert_eq!(parse_version("v0.1.2.3"), None);
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(is_newer("0.2.0", "0.1.99"));
        assert!(!is_newer("v0.1.2", "0.1.2"));
        assert!(!is_newer("v0.1.1", "0.1.2"));
        assert!(!is_newer("엉뚱한 값", "0.1.2"));
    }

    #[test]
    fn 릴리스_응답을_검사한다() {
        let ok = json!({"tag_name": "v0.1.3", "html_url": "https://github.com/JakeLim17/deskdusk/releases/tag/v0.1.3", "draft": false, "prerelease": false});
        assert_eq!(parse_release(&ok), Some(("0.1.3".into(), "https://github.com/JakeLim17/deskdusk/releases/tag/v0.1.3".into())));
        let mut other = ok.clone();
        other["html_url"] = json!("https://example.com/releases/tag/v0.1.3");
        assert_eq!(parse_release(&other), None);
        let mut pre = ok.clone();
        pre["prerelease"] = json!(true);
        assert_eq!(parse_release(&pre), None);
        assert_eq!(parse_release(&json!({"message": "Not Found"})), None);
    }

    #[test]
    fn 새_판이_있을_때만_알리고_꺼져_있으면_알리지_않는다() {
        let mut s = State { latest: Some("0.1.3".into()), url: Some(format!("{RELEASES}tag/v0.1.3")), ..Default::default() };
        let i = info_of(&s, "0.1.2");
        assert_eq!(i.newer.as_deref(), Some("0.1.3"));
        assert!(i.url.is_some());
        assert_eq!(info_of(&s, "0.1.3").newer, None);
        assert_eq!(info_of(&s, "0.1.3").url, None);
        s.off = true;
        assert_eq!(info_of(&s, "0.1.2").newer, None);
        assert!(!info_of(&s, "0.1.2").on);
    }

    #[test]
    fn 하루에_한_번만_묻고_실패하면_3시간_뒤에_묻는다() {
        let now = 1_000_000;
        assert!(due(&State::default(), now));
        let ok = State { checked_at: now - 3600, latest: Some("0.1.2".into()), ..Default::default() };
        assert!(!due(&ok, now));
        assert!(due(&State { checked_at: now - INTERVAL, ..ok.clone() }, now));
        let failed = State { checked_at: now - 3600, ..Default::default() };
        assert!(!due(&failed, now));
        assert!(due(&State { checked_at: now - RETRY, ..Default::default() }, now));
        assert!(!due(&State { off: true, ..Default::default() }, now));
        // 시계가 뒤로 갔으면 다시 묻는다.
        assert!(due(&State { checked_at: now + 999, latest: Some("0.1.2".into()), ..Default::default() }, now));
    }

    #[test]
    fn 저장하고_다시_읽는다() {
        let home = std::env::temp_dir().join(format!("deskdusk-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        assert_eq!(load_at(&home), State::default());
        let s = State { off: true, checked_at: 5, latest: Some("0.1.3".into()), url: None };
        save_at(&home, &s).unwrap();
        assert_eq!(load_at(&home), s);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn 이_저장소의_릴리스_주소만_연다() {
        assert!(open_release("https://example.com/x").is_err());
        assert!(open_release("https://github.com/JakeLim17/deskdusk/releases/tag/v1 & calc").is_err());
        assert!(open_release("https://github.com/JakeLim17/deskdusk/releases/tag/v0.1.3\"").is_err());
    }
}
