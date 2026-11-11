//! 연결 상태: 어떤 값을 읽을 수 있는지 알려 주고, Claude 한도 연결(상태줄 등록)을 대신 해 준다.
//! Claude Code는 한도 값을 상태줄로만 내주므로, 상태줄 명령에 작은 스크립트를 등록해 값을 파일로 받는다.
//! 건드리는 파일은 ~/.claude/settings.json 의 statusLine 항목과 ~/.deskdusk/statusline.mjs 뿐이고,
//! 설정 파일은 고치기 전에 항상 백업한다. 다른 상태줄을 이미 쓰고 있으면 덮어쓰지 않는다.

use crate::usage;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const BRIDGE: &str = include_str!("../../scripts/claude-statusline-bridge.mjs");
const MARK: &str = ".deskdusk/statusline.mjs";
/// 이름을 바꾸기 전(토큰 창가 시절)에 등록한 상태줄. 우리 것으로 보고 새 경로로 바꿔 준다.
const LEGACY_MARK: &str = concat!(".token", "window/statusline.mjs");

#[derive(Serialize, Debug, PartialEq)]
pub struct Status {
    /// Claude Code 기록 폴더가 있는지
    pub claude_logs: bool,
    /// Codex 기록 폴더가 있는지
    pub codex_logs: bool,
    /// "connected" | "not_connected" | "other_statusline" | "no_claude"
    pub claude_limits: String,
    /// 연결한 뒤 실제로 값을 받은 적이 있는지
    pub claude_limits_seen: bool,
    /// 상태줄 스크립트를 돌릴 Node.js를 찾았는지
    pub node: bool,
}

fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}
fn script_path(home: &Path) -> PathBuf {
    home.join(".deskdusk").join("statusline.mjs")
}

fn read_settings(home: &Path) -> Result<Value, String> {
    match fs::read_to_string(settings_path(home)) {
        Ok(s) if s.trim().is_empty() => Ok(json!({})),
        Ok(s) => serde_json::from_str::<Value>(&s).map_err(|_| "Claude 설정 파일을 읽지 못했어요(형식 오류).".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

fn status_command(v: &Value) -> Option<&str> {
    v.get("statusLine")?.get("command")?.as_str()
}

/// 우리가 등록한 상태줄인지(윈도우의 역슬래시 경로도 알아본다).
fn is_ours(command: &str) -> bool {
    let c = command.replace('\\', "/");
    c.contains(MARK) || c.contains(LEGACY_MARK)
}

/// 지금 경로로 등록돼 있는지(예전 경로면 다시 연결해야 한다).
fn is_current(command: &str) -> bool {
    command.replace('\\', "/").contains(MARK)
}

/// Node.js 실행 파일을 찾는다(흔한 설치 위치만 본다. 셸을 띄우지 않는다).
pub fn find_node(home: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = if cfg!(target_os = "windows") {
        let mut v = vec![PathBuf::from(r"C:\Program Files\nodejs\node.exe"), home.join(r"AppData\Local\Volta\bin\node.exe"), home.join(r"scoop\apps\nodejs\current\node.exe"), home.join(r"scoop\apps\nodejs-lts\current\node.exe")];
        // nvm-windows는 버전별 폴더라 가장 뒤(최신)에 있는 것을 고른다.
        if let Ok(rd) = fs::read_dir(home.join(r"AppData\Roaming\nvm")) {
            let mut vs: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("node.exe")).filter(|p| p.is_file()).collect();
            vs.sort();
            if let Some(p) = vs.pop() {
                v.insert(0, p);
            }
        }
        v
    } else {
        vec![PathBuf::from("/opt/homebrew/bin/node"), PathBuf::from("/usr/local/bin/node"), home.join(".volta/bin/node"), PathBuf::from("/usr/bin/node")]
    };
    // nvm은 버전별 폴더라 가장 뒤(최신)에 있는 것을 고른다.
    if let Ok(rd) = fs::read_dir(home.join(".nvm/versions/node")) {
        let mut vs: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin/node")).filter(|p| p.is_file()).collect();
        vs.sort();
        if let Some(p) = vs.pop() {
            found.insert(0, p);
        }
    }
    found.into_iter().find(|p| p.is_file())
}

/// 상태줄에 등록할 명령. 윈도우에서는 어떤 셸에서도 통하도록 큰따옴표와 슬래시 경로를 쓴다.
fn bridge_command(node: &Path, script: &Path) -> String {
    if cfg!(target_os = "windows") {
        let fwd = |p: &Path| p.display().to_string().replace('\\', "/");
        format!("\"{}\" \"{}\"", fwd(node), fwd(script))
    } else {
        format!("'{}' '{}'", node.display(), script.display())
    }
}

pub fn status_at(home: &Path) -> Status {
    let claude_dir = home.join(".claude").is_dir();
    let cmd = read_settings(home).ok().and_then(|v| status_command(&v).map(str::to_string));
    let claude_limits = if !claude_dir {
        "no_claude"
    } else {
        match cmd.as_deref() {
            Some(c) if is_current(c) && script_path(home).is_file() => "connected",
            Some(c) if !is_ours(c) && !c.trim().is_empty() => "other_statusline",
            _ => "not_connected",
        }
    };
    Status {
        claude_logs: home.join(".claude").join("projects").is_dir(),
        codex_logs: home.join(".codex").join("sessions").is_dir(),
        claude_limits: claude_limits.to_string(),
        claude_limits_seen: home.join(".deskdusk").join("claude-limits.json").is_file(),
        node: find_node(home).is_some(),
    }
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("deskdusk-tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// 설정 파일이 있으면 같은 폴더에 백업을 남긴다.
fn backup(home: &Path, stamp: &str) -> Result<(), String> {
    let p = settings_path(home);
    if p.is_file() {
        fs::copy(&p, p.with_file_name(format!("settings.json.bak-deskdusk-{stamp}"))).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn save_settings(home: &Path, v: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    write_atomic(&settings_path(home), &text)
}

/// Claude 한도 연결: 스크립트를 두고 상태줄 명령으로 등록한다.
pub fn connect_at(home: &Path, stamp: &str) -> Result<Status, String> {
    if !home.join(".claude").is_dir() {
        return Err("Claude Code를 먼저 설치하고 한 번 실행해 주세요.".into());
    }
    let node = find_node(home).ok_or("Node.js를 찾지 못했어요. Node.js를 설치한 뒤 다시 시도해 주세요.")?;
    let mut v = read_settings(home)?;
    if !v.is_object() {
        return Err("Claude 설정 파일을 읽지 못했어요(형식 오류).".into());
    }
    if let Some(c) = status_command(&v) {
        if !is_ours(c) && !c.trim().is_empty() {
            return Err("이미 다른 상태줄을 쓰고 있어 덮어쓰지 않았어요. README의 안내대로 직접 연결할 수 있어요.".into());
        }
    }
    write_atomic(&script_path(home), BRIDGE)?;
    backup(home, stamp)?;
    let command = bridge_command(&node, &script_path(home));
    v["statusLine"] = json!({ "type": "command", "command": command });
    save_settings(home, &v)?;
    Ok(status_at(home))
}

/// 연결 끊기: 우리가 등록한 상태줄일 때만 지운다.
pub fn disconnect_at(home: &Path, stamp: &str) -> Result<Status, String> {
    let mut v = read_settings(home)?;
    if status_command(&v).is_some_and(is_ours) {
        backup(home, stamp)?;
        if let Some(o) = v.as_object_mut() {
            o.remove("statusLine");
        }
        save_settings(home, &v)?;
    }
    Ok(status_at(home))
}

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}
pub fn status() -> Status {
    status_at(&usage::home())
}
pub fn connect() -> Result<Status, String> {
    connect_at(&usage::home(), &stamp())
}
pub fn disconnect() -> Result<Status, String> {
    disconnect_at(&usage::home(), &stamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 가짜 홈 폴더. Node.js가 있는 것처럼 보이게 빈 파일을 둔다.
    fn home(name: &str, with_node: bool) -> PathBuf {
        let d = std::env::temp_dir().join(format!("deskdusk-connect-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join(".claude")).unwrap();
        if with_node {
            fs::create_dir_all(d.join(".volta/bin")).unwrap();
            fs::write(d.join(".volta/bin/node"), "").unwrap();
        }
        d
    }
    fn settings(h: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(settings_path(h)).unwrap()).unwrap()
    }
    /// 시험 기계에 실제 Node.js가 있으면 가짜 홈보다 먼저 잡히므로, 명령에 스크립트 경로가 들어갔는지만 본다.
    fn ours(h: &Path) -> bool {
        status_command(&settings(h)).is_some_and(|c| is_ours(c) && c.contains("node"))
    }

    #[test]
    fn 연결하면_다른_설정은_그대로_두고_상태줄만_등록하며_백업을_남긴다() {
        let h = home("connect", true);
        fs::write(settings_path(&h), r#"{"zeta":1,"model":"opus","alpha":{"b":2,"a":1}}"#).unwrap();
        assert_eq!(status_at(&h).claude_limits, "not_connected");
        let st = connect_at(&h, "t1").unwrap();
        assert_eq!(st.claude_limits, "connected");
        assert!(ours(&h) && script_path(&h).is_file());
        let v = settings(&h);
        assert_eq!((v["zeta"].clone(), v["model"].clone(), v["alpha"]["b"].clone()), (json!(1), json!("opus"), json!(2)));
        // 열쇠 순서도 그대로(맨 뒤에 statusLine만 붙는다)
        assert_eq!(v.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["zeta", "model", "alpha", "statusLine"]);
        let bak = fs::read_to_string(h.join(".claude/settings.json.bak-deskdusk-t1")).unwrap();
        assert_eq!(bak, r#"{"zeta":1,"model":"opus","alpha":{"b":2,"a":1}}"#);
        // 다시 눌러도 같은 상태
        assert_eq!(connect_at(&h, "t2").unwrap().claude_limits, "connected");
        let _ = fs::remove_dir_all(&h);
    }

    #[test]
    fn 설정_파일이_없어도_연결된다() {
        let h = home("fresh", true);
        assert_eq!(connect_at(&h, "t").unwrap().claude_limits, "connected");
        assert!(ours(&h));
        let _ = fs::remove_dir_all(&h);
    }

    #[test]
    fn 다른_상태줄을_쓰고_있으면_덮어쓰지_않는다() {
        let h = home("other", true);
        let before = r#"{"statusLine":{"type":"command","command":"my-status --fancy"}}"#;
        fs::write(settings_path(&h), before).unwrap();
        assert_eq!(status_at(&h).claude_limits, "other_statusline");
        assert!(connect_at(&h, "t").is_err());
        assert_eq!(fs::read_to_string(settings_path(&h)).unwrap(), before);
        // 연결 끊기도 남의 상태줄은 건드리지 않는다.
        disconnect_at(&h, "t").unwrap();
        assert_eq!(fs::read_to_string(settings_path(&h)).unwrap(), before);
        let _ = fs::remove_dir_all(&h);
    }

    #[test]
    fn 형식이_깨진_설정_파일은_건드리지_않는다() {
        let h = home("broken", true);
        fs::write(settings_path(&h), "{ 깨진 내용").unwrap();
        assert!(connect_at(&h, "t").is_err());
        assert_eq!(fs::read_to_string(settings_path(&h)).unwrap(), "{ 깨진 내용");
        let _ = fs::remove_dir_all(&h);
    }

    #[test]
    fn 연결을_끊으면_상태줄만_지우고_나머지는_남긴다() {
        let h = home("off", true);
        fs::write(settings_path(&h), r#"{"model":"opus"}"#).unwrap();
        connect_at(&h, "a").unwrap();
        let st = disconnect_at(&h, "b").unwrap();
        assert_eq!(st.claude_limits, "not_connected");
        let v = settings(&h);
        assert!(v.get("statusLine").is_none() && v["model"] == "opus");
        let _ = fs::remove_dir_all(&h);
    }

    #[test]
    fn 클로드가_없으면_연결하지_않는다() {
        let d = std::env::temp_dir().join(format!("deskdusk-connect-{}-none", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert_eq!(status_at(&d).claude_limits, "no_claude");
        assert!(connect_at(&d, "t").is_err());
        assert!(!settings_path(&d).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn 역슬래시로_적힌_윈도우_명령도_우리_것으로_알아본다() {
        assert!(is_ours(r#""C:/Program Files/nodejs/node.exe" "C:/Users/a/.deskdusk/statusline.mjs""#));
        assert!(is_ours(r"node C:\Users\a\.deskdusk\statusline.mjs"));
        assert!(!is_ours("my-status --fancy"));
    }

    #[test]
    fn 예전_이름으로_등록한_상태줄은_새_경로로_바꿔_준다() {
        let h = home("legacy", true);
        let old = format!(r#"{{"model":"opus","statusLine":{{"type":"command","command":"'node' '/Users/a/{}'"}}}}"#, LEGACY_MARK);
        fs::write(settings_path(&h), &old).unwrap();
        assert_eq!(status_at(&h).claude_limits, "not_connected");
        assert_eq!(connect_at(&h, "t").unwrap().claude_limits, "connected");
        let cmd = status_command(&settings(&h)).unwrap().to_string();
        assert!(is_current(&cmd) && !cmd.contains(LEGACY_MARK));
        assert_eq!(settings(&h)["model"], "opus");
        let _ = fs::remove_dir_all(&h);
    }
}
