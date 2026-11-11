//! 바탕화면 위젯: 테두리 없는 작은 창을 모든 데스크톱에 띄운다(기본은 다른 창 위, 고르면 다른 창 뒤 바탕화면).
//! 여러 개를 띄울 수 있고, 위젯마다 위치·크기와 꾸밈(모양, 보여 줄 항목, 서비스, 풍경)을 따로 기억한다.
//! 설정은 ~/.deskdusk/widget.json 에 둔다. 꾸밈(opts)은 화면 쪽이 정하는 값이라 본체는 그대로 저장만 한다.

use crate::usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, Wry};

pub const LABEL_PREFIX: &str = "widget-";
/// 위젯 메뉴 항목 번호 앞에 붙는 표시. 뒤에 「창 이름|항목」이 온다.
pub const MENU_PREFIX: &str = "wm|";
const MIN_SIZE: (f64, f64) = (200.0, 56.0);
const MAX_WIDGETS: usize = 8;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub w: Option<f64>,
    pub h: Option<f64>,
    #[serde(default)]
    pub opts: Value,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub on: bool,
    #[serde(default)]
    pub widgets: Vec<Item>,
}

fn path(home: &Path) -> PathBuf {
    home.join(".deskdusk").join("widget.json")
}

/// 파일이 없거나 깨져 있으면 기본값(꺼짐). 위젯이 하나뿐이던 예전 형식도 읽는다.
pub fn load_at(home: &Path) -> Config {
    let Some(v) = std::fs::read_to_string(path(home)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { return Config::default() };
    if v.get("widgets").is_some() {
        let mut c: Config = serde_json::from_value(v).unwrap_or_default();
        c.widgets.retain(|i| is_valid_id(&i.id));
        c.widgets.truncate(MAX_WIDGETS);
        return c;
    }
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    let had_place = ["x", "y", "w", "h"].iter().any(|k| num(k).is_some());
    let on = v["on"] == true;
    let widgets = if on || had_place { vec![Item { id: "1".into(), x: num("x"), y: num("y"), w: num("w"), h: num("h"), opts: Value::Null }] } else { Vec::new() };
    Config { on, widgets }
}

pub fn save_at(home: &Path, c: &Config) -> Result<(), String> {
    let p = path(home);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 8 && id.bytes().all(|b| b.is_ascii_digit())
}

/// 모양에 맞는 처음 크기. 「숫자 띠」는 낮고 넓다.
fn default_size(opts: &Value) -> (f64, f64) {
    match opts["shape"].as_str() {
        Some("strip") => (460.0, 92.0),
        _ => (440.0, 275.0),
    }
}

/// 저장된 크기를 쓸 수 있는 범위로 맞춘다(너무 작거나 이상한 값 방지).
pub fn size_of(i: &Item) -> (f64, f64) {
    let def = default_size(&i.opts);
    let ok = |v: Option<f64>, min: f64, d: f64| match v {
        Some(v) if v.is_finite() && v >= min && v <= 4000.0 => v,
        _ => d,
    };
    (ok(i.w, MIN_SIZE.0, def.0), ok(i.h, MIN_SIZE.1, def.1))
}

/// 꾸밈에서 「항상 위에 두기」가 켜져 있는지. 기본은 켜짐(다른 창 위에 떠서 바로 보인다)이고,
/// 사용자가 끈 위젯만 다른 창 뒤 바탕화면에 깔린다.
pub fn on_top(opts: &Value) -> bool {
    opts["top"] != false
}

/// 꾸밈의 「항상 위에 두기」 값만 바꾼다(다른 꾸밈은 그대로).
pub fn with_top(opts: &Value, top: bool) -> Value {
    let mut o = if opts.is_object() { opts.clone() } else { serde_json::json!({}) };
    o["top"] = Value::Bool(top);
    o
}

/// 모든 위젯이 다른 창 위에 떠 있는지(메뉴바 메뉴의 체크 표시).
pub fn all_top(c: &Config) -> bool {
    c.widgets.iter().all(|i| on_top(&i.opts))
}

pub fn is_all_top() -> bool {
    all_top(&load_at(&usage::home()))
}

/// 새 위젯의 번호: 지금 있는 가장 큰 번호 다음.
pub fn next_id(c: &Config) -> String {
    (c.widgets.iter().filter_map(|i| i.id.parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string()
}

/// 위젯을 하나 더한다. 가장 최근 위젯에서 조금 비켜난 자리에 둔다. 꽉 찼으면 None.
pub fn add_to(c: &mut Config) -> Option<Item> {
    if c.widgets.len() >= MAX_WIDGETS {
        return None;
    }
    let (x, y) = match c.widgets.last() {
        Some(Item { x: Some(x), y: Some(y), .. }) => (Some(x + 36.0), Some(y + 36.0)),
        _ => (None, None),
    };
    let item = Item { id: next_id(c), x, y, w: None, h: None, opts: Value::Null };
    c.widgets.push(item.clone());
    c.on = true;
    Some(item)
}

/// 위젯을 뺀다. 하나도 안 남으면 꺼짐으로 둔다.
pub fn remove_from(c: &mut Config, id: &str) {
    c.widgets.retain(|i| i.id != id);
    if c.widgets.is_empty() {
        c.on = false;
    }
}

pub fn id_of(label: &str) -> Option<&str> {
    label.strip_prefix(LABEL_PREFIX).filter(|id| is_valid_id(id))
}

pub fn is_on() -> bool {
    load_at(&usage::home()).on
}

fn trace(msg: &str) {
    if std::env::var_os("DESKDUSK_TRACE").is_some() {
        eprintln!("[trace] {msg}");
    }
}

fn open(app: &tauri::AppHandle, item: &Item) -> Result<(), String> {
    let label = format!("{LABEL_PREFIX}{}", item.id);
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }
    let (w, h) = size_of(item);
    let info = serde_json::json!({ "id": item.id, "opts": item.opts });
    let mut b = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("DeskDusk 위젯")
        // 같은 화면 코드가 위젯 모양으로 그리도록 번호와 꾸밈을 넘겨 준다.
        .initialization_script(&format!("window.__TW_WIDGET = {info};"))
        .inner_size(w, h)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .decorations(false)
        .resizable(true)
        .always_on_bottom(!on_top(&item.opts))
        .always_on_top(on_top(&item.opts))
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false);
    if let (Some(x), Some(y)) = (item.x, item.y) {
        if x.is_finite() && y.is_finite() {
            b = b.position(x, y);
        }
    }
    let built = b.build().map_err(|e| e.to_string());
    let result = match built {
        Ok(win) => {
            // 만들 때 준 설정이 다른 창 설정에 덮일 수 있어, 만든 뒤에 한 번 더 건다:
            // 모든 데스크톱(Space)에 보이고, 다른 창 뒤에 깔린다.
            let all = win.set_visible_on_all_workspaces(true);
            let top = on_top(&item.opts);
            let level = set_level(&win, top);
            trace(&format!("위젯 창 {}: 모든 데스크톱 {all:?}, 항상 위에 {top} {level:?}", item.id));
            Ok(())
        }
        Err(e) => Err(e),
    };
    trace(&format!("위젯 창 열기 {}: {result:?} ({w}x{h})", item.id));
    result
}

/// 처음 실행(설정 파일이 아직 없음)이면 위젯 하나를 켠 상태로 시작한다.
/// 이 앱의 핵심이 바탕화면 위젯이라, 설치하고 처음 켰을 때 바로 보이게 한다. 처음이었는지를 돌려준다.
pub fn init_first_run_at(home: &Path) -> bool {
    if path(home).exists() {
        return false;
    }
    let cfg = Config { on: true, widgets: vec![Item { id: "1".into(), ..Default::default() }] };
    save_at(home, &cfg).is_ok()
}

pub fn init_first_run() -> bool {
    init_first_run_at(&usage::home())
}

/// 창의 높이를 정한다. 「위」와 「아래」는 서로 덮어쓰므로, 끄는 쪽을 먼저 하고 켜는 쪽을 나중에 한다.
pub fn set_level<W: tauri::Runtime>(win: &tauri::WebviewWindow<W>, top: bool) -> tauri::Result<()> {
    if top {
        win.set_always_on_bottom(false)?;
        win.set_always_on_top(true)
    } else {
        win.set_always_on_top(false)?;
        win.set_always_on_bottom(true)
    }
}

/// 켜져 있으면 저장된 위젯을 모두 띄운다.
pub fn open_all(app: &tauri::AppHandle) {
    let cfg = load_at(&usage::home());
    if cfg.on {
        for item in &cfg.widgets {
            let _ = open(app, item);
        }
    }
}

fn close_all(app: &tauri::AppHandle) {
    for (label, w) in app.webview_windows() {
        if label.starts_with(LABEL_PREFIX) {
            let _ = w.destroy();
        }
    }
}

/// 전체를 켜거나 끄고 그 상태를 기억한다. 켤 때 위젯이 하나도 없으면 하나 만든다.
pub fn set(app: &tauri::AppHandle, on: bool) -> Result<bool, String> {
    let home = usage::home();
    let mut cfg = load_at(&home);
    cfg.on = on;
    if on && cfg.widgets.is_empty() {
        cfg.widgets.push(Item { id: "1".into(), ..Default::default() });
    }
    save_at(&home, &cfg)?;
    if on {
        open_all(app);
        flash_all(app);
    } else {
        close_all(app);
    }
    Ok(on)
}

/// 바탕화면에 깔아 둔 위젯을 잠깐 다른 창 위로 올렸다가 내린다.
/// 창이 화면을 덮고 있어도 켜거나 옮긴 것이 눈에 보이게 하려는 것이다.
pub fn flash<W: tauri::Runtime>(win: &tauri::WebviewWindow<W>) {
    let Some(id) = id_of(win.label()).map(str::to_string) else { return };
    let top_of = move || load_at(&usage::home()).widgets.iter().find(|i| i.id == id).map(|i| on_top(&i.opts));
    if top_of() != Some(false) {
        return;
    }
    let _ = set_level(win, true);
    let win = win.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        // 그 사이에 사용자가 「항상 위에 두기」를 켰으면 내리지 않는다.
        if top_of() == Some(false) {
            let _ = set_level(&win, false);
            let _ = win.set_visible_on_all_workspaces(true);
        }
    });
}

fn flash_all(app: &tauri::AppHandle) {
    for (label, w) in app.webview_windows() {
        if label.starts_with(LABEL_PREFIX) {
            flash(&w);
        }
    }
}

/// 모든 위젯을 다른 창 위에 띄우거나(top) 바탕화면에 깐다. 열려 있는 위젯 화면에도 알려 준다.
pub fn set_all_top(app: &tauri::AppHandle, top: bool) -> Result<(), String> {
    let home = usage::home();
    let mut cfg = load_at(&home);
    for i in cfg.widgets.iter_mut() {
        i.opts = with_top(&i.opts, top);
    }
    save_at(&home, &cfg)?;
    for (label, w) in app.webview_windows() {
        if label.starts_with(LABEL_PREFIX) {
            let _ = set_level(&w, top);
            let _ = w.set_visible_on_all_workspaces(true);
            let _ = w.eval(&format!("window.__twSetTop && window.__twSetTop({top})"));
        }
    }
    Ok(())
}

/// 위젯을 하나 더 띄운다.
pub fn add(app: &tauri::AppHandle) -> Result<(), String> {
    let home = usage::home();
    let mut cfg = load_at(&home);
    let item = add_to(&mut cfg).ok_or("위젯은 8개까지 띄울 수 있어요.")?;
    save_at(&home, &cfg)?;
    open_all(app);
    let _ = item;
    Ok(())
}

/// 위젯 하나를 닫고 목록에서 뺀다.
pub fn remove(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let Some(id) = id_of(label) else { return Ok(()) };
    let home = usage::home();
    let mut cfg = load_at(&home);
    remove_from(&mut cfg, id);
    save_at(&home, &cfg)?;
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.destroy();
    }
    Ok(())
}

/// 위젯의 꾸밈을 기억한다.
pub fn set_opts(label: &str, opts: Value) -> Result<(), String> {
    let Some(id) = id_of(label) else { return Ok(()) };
    let home = usage::home();
    let mut cfg = load_at(&home);
    if let Some(i) = cfg.widgets.iter_mut().find(|i| i.id == id) {
        i.opts = opts;
        save_at(&home, &cfg)?;
    }
    Ok(())
}

/// 위젯 하나의 「항상 위에 두기」 값을 기억한다.
pub fn set_top_of(label: &str, top: bool) -> Result<(), String> {
    let Some(id) = id_of(label) else { return Ok(()) };
    let home = usage::home();
    let mut cfg = load_at(&home);
    if let Some(i) = cfg.widgets.iter_mut().find(|i| i.id == id) {
        i.opts = with_top(&i.opts, top);
        save_at(&home, &cfg)?;
    }
    Ok(())
}

/// 위젯을 옮기거나 크기를 바꿨을 때 위치·크기를 기억한다.
pub fn remember(window: &tauri::Window) {
    let Some(id) = id_of(window.label()) else { return };
    let (Ok(pos), Ok(size), Ok(scale)) = (window.outer_position(), window.inner_size(), window.scale_factor()) else { return };
    let home = usage::home();
    let mut cfg = load_at(&home);
    let Some(i) = cfg.widgets.iter_mut().find(|i| i.id == id) else { return };
    let (p, s) = (pos.to_logical::<f64>(scale), size.to_logical::<f64>(scale));
    (i.x, i.y, i.w, i.h) = (Some(p.x), Some(p.y), Some(s.width), Some(s.height));
    let _ = save_at(&home, &cfg);
}

/// 화면이 넘겨준 메뉴 설명으로 시스템 메뉴를 만든다.
/// 항목: {"id","label","checked"?} / 하위 메뉴: {"label","items":[…]} / 구분선: {"sep":true}
fn build_items(app: &tauri::AppHandle, label: &str, spec: &[Value], depth: u8) -> Result<Vec<Box<dyn IsMenuItem<Wry>>>, String> {
    let mut out: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for (n, it) in spec.iter().take(40).enumerate() {
        let text = it["label"].as_str().unwrap_or("");
        if it["sep"] == true {
            out.push(Box::new(PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?));
        } else if let Some(children) = it["items"].as_array() {
            if depth >= 2 {
                continue;
            }
            let kids = build_items(app, label, children, depth + 1)?;
            let refs: Vec<&dyn IsMenuItem<Wry>> = kids.iter().map(|b| b.as_ref()).collect();
            out.push(Box::new(Submenu::with_id_and_items(app, format!("{MENU_PREFIX}{label}|sub{depth}-{n}"), text, true, &refs).map_err(|e| e.to_string())?));
        } else if let Some(id) = it["id"].as_str() {
            let mid = format!("{MENU_PREFIX}{label}|{id}");
            match it["checked"].as_bool() {
                Some(c) => out.push(Box::new(CheckMenuItem::with_id(app, mid, text, true, c, None::<&str>).map_err(|e| e.to_string())?)),
                None => out.push(Box::new(MenuItem::with_id(app, mid, text, true, None::<&str>).map_err(|e| e.to_string())?)),
            }
        }
    }
    Ok(out)
}

/// 위젯을 오른쪽 클릭했을 때 메뉴를 띄운다. 고른 항목은 그 위젯 화면으로 돌려보낸다(lib.rs의 메뉴 처리).
pub fn popup(window: &tauri::WebviewWindow, spec: &Value) -> Result<(), String> {
    let app = window.app_handle();
    let items = build_items(app, window.label(), spec.as_array().map(Vec::as_slice).unwrap_or(&[]), 0)?;
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
    let menu = Menu::with_items(app, &refs).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// 메뉴 항목 번호에서 (창 이름, 항목)을 꺼낸다.
pub fn menu_target(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(MENU_PREFIX)?.split_once('|')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn home(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("deskdusk-widget-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".deskdusk")).unwrap();
        d
    }

    #[test]
    fn 설정은_저장했다가_그대로_읽힌다() {
        let h = home("roundtrip");
        assert_eq!(load_at(&h), Config::default());
        let c = Config { on: true, widgets: vec![Item { id: "2".into(), x: Some(12.5), y: Some(-40.0), w: Some(500.0), h: Some(300.0), opts: json!({"shape": "strip", "items": ["bars"]}) }] };
        save_at(&h, &c).unwrap();
        assert_eq!(load_at(&h), c);
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn 위젯이_하나뿐이던_예전_형식도_읽는다() {
        let h = home("legacy");
        std::fs::write(path(&h), r#"{"on":true,"x":10,"y":20,"w":480,"h":300}"#).unwrap();
        let c = load_at(&h);
        assert!(c.on);
        assert_eq!(c.widgets, vec![Item { id: "1".into(), x: Some(10.0), y: Some(20.0), w: Some(480.0), h: Some(300.0), opts: Value::Null }]);
        std::fs::write(path(&h), r#"{"on":false,"x":null,"y":null,"w":null,"h":null}"#).unwrap();
        assert_eq!(load_at(&h), Config::default());
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn 깨진_파일이나_이상한_번호는_버린다() {
        let h = home("broken");
        std::fs::write(path(&h), "{ 깨짐").unwrap();
        assert_eq!(load_at(&h), Config::default());
        std::fs::write(path(&h), r#"{"on":true,"widgets":[{"id":"../x"},{"id":"3"},{"id":""}]}"#).unwrap();
        assert_eq!(load_at(&h).widgets.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["3"]);
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn 크기는_모양에_맞는_기본값과_쓸_수_있는_범위를_따른다() {
        let plain = Item { id: "1".into(), ..Default::default() };
        assert_eq!(size_of(&plain), (440.0, 275.0));
        let strip = Item { id: "1".into(), opts: json!({"shape": "strip"}), ..Default::default() };
        assert_eq!(size_of(&strip), (460.0, 92.0));
        assert_eq!(size_of(&Item { w: Some(10.0), h: Some(f64::NAN), ..plain.clone() }), (440.0, 275.0));
        assert_eq!(size_of(&Item { w: Some(600.0), h: Some(80.0), ..strip }), (600.0, 80.0));
    }

    #[test]
    fn 위젯을_더하고_빼면_번호와_켜짐_상태가_맞게_바뀐다() {
        let mut c = Config::default();
        assert_eq!(add_to(&mut c).unwrap().id, "1");
        c.widgets[0].x = Some(100.0);
        c.widgets[0].y = Some(50.0);
        let second = add_to(&mut c).unwrap();
        assert_eq!((second.id.as_str(), second.x, second.y), ("2", Some(136.0), Some(86.0)));
        assert!(c.on);
        remove_from(&mut c, "1");
        assert_eq!(add_to(&mut c).unwrap().id, "3");
        remove_from(&mut c, "2");
        remove_from(&mut c, "3");
        assert!(c.widgets.is_empty() && !c.on);
        for _ in 0..20 {
            add_to(&mut c);
        }
        assert_eq!(c.widgets.len(), MAX_WIDGETS);
    }

    #[test]
    fn 창_이름과_메뉴_번호를_풀어낸다() {
        assert_eq!(id_of("widget-12"), Some("12"));
        assert_eq!(id_of("widget-x"), None);
        assert_eq!(id_of("main"), None);
        assert_eq!(menu_target("wm|widget-2|shape:strip"), Some(("widget-2", "shape:strip")));
        assert_eq!(menu_target("show"), None);
    }

    #[test]
    fn 처음_실행이면_위젯_하나를_켠_상태로_시작하고_두_번째부터는_건드리지_않는다() {
        let h = home("first");
        let _ = std::fs::remove_file(path(&h));
        assert!(init_first_run_at(&h));
        let c = load_at(&h);
        assert!(c.on && c.widgets.len() == 1 && c.widgets[0].id == "1");
        // 사용자가 끈 뒤에는 다시 켜지 않는다.
        save_at(&h, &Config::default()).unwrap();
        assert!(!init_first_run_at(&h));
        assert!(!load_at(&h).on);
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn 항상_위에_두기는_꾸밈에서_켰을_때만() {
        // 기본은 다른 창 위. 사용자가 끈 것만 바탕화면에 깔린다.
        assert!(on_top(&Value::Null));
        assert!(on_top(&json!({"shape": "strip"})));
        assert!(on_top(&json!({"top": true})));
        assert!(!on_top(&json!({"top": false})));
        let o = with_top(&json!({"shape": "strip"}), false);
        assert_eq!(o, json!({"shape": "strip", "top": false}));
        assert_eq!(with_top(&Value::Null, true), json!({"top": true}));
        let mut c = Config { on: true, widgets: vec![Item { id: "1".into(), ..Default::default() }, Item { id: "2".into(), opts: o, ..Default::default() }] };
        assert!(!all_top(&c));
        c.widgets[1].opts = with_top(&c.widgets[1].opts, true);
        assert!(all_top(&c));
    }
}
