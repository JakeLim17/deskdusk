//! 로그인할 때 자동 실행: 맥은 로그인 항목(LaunchAgent) 파일을 직접 쓰고 지우고, 윈도우는 시작 프로그램 등록 값을 넣고 뺀다.
//! 기본은 꺼짐이고, 사용자가 설정에서 켰을 때만 파일을 만든다.

use crate::usage;
use std::path::{Path, PathBuf};

const LABEL: &str = "dev.chronocode.deskdusk.autostart";
/// 자동 실행으로 켜질 때는 창을 띄우지 않고 메뉴바에만 올린다.
pub const HIDDEN_ARG: &str = "--hidden";

fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents").join(format!("{LABEL}.plist"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn plist_text(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>{HIDDEN_ARG}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#,
        xml_escape(exe)
    )
}

pub fn is_enabled_at(home: &Path) -> bool {
    plist_path(home).is_file()
}

pub fn enable_at(home: &Path, exe: &str) -> Result<(), String> {
    let p = plist_path(home);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // 쓰다 만 파일이 남지 않게 임시 파일에 쓴 뒤 이름을 바꾼다.
    let tmp = p.with_extension("plist.tmp");
    std::fs::write(&tmp, plist_text(exe)).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

pub fn disable_at(home: &Path) -> Result<(), String> {
    match std::fs::remove_file(plist_path(home)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// 윈도우: 사용자 시작 프로그램 등록(HKCU …\\Run)에 값을 넣고 뺀다. 창이 깜빡이지 않게 reg.exe를 창 없이 부른다.
#[cfg(target_os = "windows")]
mod win {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "DeskDusk";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn reg(args: &[&str]) -> Result<bool, String> {
        Command::new("reg").args(args).creation_flags(CREATE_NO_WINDOW).output().map(|o| o.status.success()).map_err(|e| e.to_string())
    }
    pub fn is_enabled() -> bool {
        reg(&["query", KEY, "/v", NAME]).unwrap_or(false)
    }
    pub fn set(on: bool, exe: &str) -> Result<(), String> {
        if on {
            let value = format!("\"{exe}\" {}", super::HIDDEN_ARG);
            if !reg(&["add", KEY, "/v", NAME, "/t", "REG_SZ", "/d", &value, "/f"])? {
                return Err("시작 프로그램에 등록하지 못했어요.".into());
            }
        } else if is_enabled() {
            reg(&["delete", KEY, "/v", NAME, "/f"])?;
        }
        Ok(())
    }
}

pub fn is_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        win::is_enabled()
    }
    #[cfg(not(target_os = "windows"))]
    {
        cfg!(target_os = "macos") && is_enabled_at(&usage::home())
    }
}

/// 켜거나 끄고, 바뀐 뒤의 상태를 돌려준다.
pub fn set(on: bool) -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_str().ok_or("앱 경로를 읽지 못했어요.")?;
    #[cfg(target_os = "windows")]
    {
        win::set(on, exe)?;
        Ok(win::is_enabled())
    }
    #[cfg(not(target_os = "windows"))]
    {
        if !cfg!(target_os = "macos") {
            return Err("자동 실행은 맥과 윈도우에서만 동작해요.".into());
        }
        let home = usage::home();
        if on {
            enable_at(&home, exe)?;
        } else {
            disable_at(&home)?;
        }
        Ok(is_enabled_at(&home))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("deskdusk-autostart-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn 켜면_파일이_생기고_끄면_사라진다() {
        let home = temp_home("toggle");
        assert!(!is_enabled_at(&home));
        enable_at(&home, "/Applications/DeskDusk.app/Contents/MacOS/deskdusk").unwrap();
        assert!(is_enabled_at(&home));
        let text = std::fs::read_to_string(plist_path(&home)).unwrap();
        assert!(text.contains("<string>/Applications/DeskDusk.app/Contents/MacOS/deskdusk</string>"));
        assert!(text.contains("<string>--hidden</string>"));
        assert!(text.contains("<key>RunAtLoad</key><true/>"));
        disable_at(&home).unwrap();
        assert!(!is_enabled_at(&home));
        // 이미 꺼져 있어도 오류가 아니다.
        disable_at(&home).unwrap();
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn 경로의_특수_문자는_이스케이프한다() {
        let home = temp_home("escape");
        enable_at(&home, "/Apps/A&B <x>/deskdusk").unwrap();
        let text = std::fs::read_to_string(plist_path(&home)).unwrap();
        assert!(text.contains("/Apps/A&amp;B &lt;x&gt;/deskdusk"));
        assert!(!text.contains("A&B"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn 다시_켜면_경로가_새로_적힌다() {
        let home = temp_home("rewrite");
        enable_at(&home, "/old/deskdusk").unwrap();
        enable_at(&home, "/new/deskdusk").unwrap();
        let text = std::fs::read_to_string(plist_path(&home)).unwrap();
        assert!(text.contains("/new/deskdusk") && !text.contains("/old/deskdusk"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
