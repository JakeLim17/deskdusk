pub mod autostart;
pub mod claude_api;
pub mod connect;
pub mod pricing;
pub mod tray_badge;
pub mod update;
pub mod usage;
pub mod widget;

use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

/// 로그 읽기는 파일 크기에 따라 오래 걸릴 수 있어 화면이 멈추지 않도록 별도 스레드에서 돈다.
#[tauri::command]
async fn get_usage() -> Result<usage::Usage, String> {
    trace("get_usage 호출");
    tauri::async_runtime::spawn_blocking(usage::collect)
        .await
        .map_err(|e| e.to_string())
}

/// 점검용: DESKDUSK_TRACE 환경 변수가 있으면 화면이 보낸 호출을 표준 오류로 남긴다.
fn trace(msg: &str) {
    if std::env::var_os("DESKDUSK_TRACE").is_some() {
        eprintln!("[trace] {msg}");
    }
}

/// 트레이 아이콘에 그릴 숫자: 가장 많이 찬 한도의 퍼센트와 그 서비스의 색("#rrggbb").
#[derive(serde::Deserialize, Debug)]
struct Badge {
    pct: f64,
    color: String,
}

#[tauri::command]
fn set_tray_title(app: tauri::AppHandle, title: String, badge: Option<Badge>) {
    trace(&format!("set_tray_title: {title} / {badge:?}"));
    if let Some(tray) = app.tray_by_id("main") {
        // 윈도우 트레이에는 글자를 올릴 수 없어 풍선 도움말에도 같은 내용을 넣는다.
        let _ = tray.set_tooltip(Some(format!("DeskDusk · {title}")));
        let _ = tray.set_title(Some(title));
        // 윈도우에서는 아이콘 자체를 숫자로 바꾼다. 한도 값이 없으면 원래 아이콘으로 돌린다.
        if cfg!(target_os = "windows") {
            let icon = match badge.as_ref().and_then(|b| tray_badge::parse_color(&b.color).map(|c| (b.pct, c))) {
                Some((pct, color)) => Some(tauri::image::Image::new_owned(tray_badge::render(pct, color), tray_badge::SIZE, tray_badge::SIZE)),
                None => app.default_window_icon().cloned(),
            };
            let set = tray.set_icon(icon);
            trace(&format!("트레이 숫자 아이콘: {set:?}"));
        }
    }
}

/// 로그인할 때 자동 실행이 켜져 있는지.
#[tauri::command]
fn get_autostart() -> bool {
    autostart::is_enabled()
}

/// 자동 실행을 켜거나 끄고, 바뀐 뒤의 상태를 돌려준다.
#[tauri::command]
fn set_autostart(on: bool) -> Result<bool, String> {
    trace(&format!("set_autostart: {on}"));
    autostart::set(on)
}

/// 어떤 값을 읽을 수 있는지(연결 상태).
#[tauri::command]
fn get_connect_status() -> connect::Status {
    connect::status()
}

/// Claude 한도 연결을 켜거나 끈다(Claude 설정의 상태줄 항목). 고치기 전에 백업한다.
#[tauri::command]
fn set_claude_connect(on: bool) -> Result<connect::Status, String> {
    trace(&format!("set_claude_connect: {on}"));
    if on {
        connect::connect()
    } else {
        connect::disconnect()
    }
}

/// 앱이 어디서 실행 중인지 경로로 판단한다(맥).
/// "dmg": 디스크 이미지를 열어 그 안에서 바로 실행(/Volumes/<디스크>/<앱>.app). 디스크를 꺼내면 꺼지고 다음 판으로 바꿀 때 헷갈린다.
/// "translocated": 내려받기 폴더 등에서 바로 열어 macOS가 임시 위치로 옮겨 실행. "ok": 그 밖(응용 프로그램 폴더 등).
pub fn place_of(exe: &str) -> &'static str {
    let parts: Vec<&str> = exe.split('/').collect();
    // ["", "Volumes", "<디스크>", "<앱>.app", ...]
    if parts.len() > 3 && parts[1] == "Volumes" && parts[3].ends_with(".app") {
        "dmg"
    } else if exe.contains("/AppTranslocation/") {
        "translocated"
    } else {
        "ok"
    }
}

#[tauri::command]
fn install_place() -> &'static str {
    if !cfg!(target_os = "macos") {
        return "ok";
    }
    std::env::current_exe().map(|p| place_of(&p.to_string_lossy())).unwrap_or("ok")
}

/// 한도 경고 알림을 보낸다. 화면의 「알림」 설정이 켜져 있을 때만 호출된다.
#[tauri::command]
fn notify(app: tauri::AppHandle, title: String, body: String) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    trace(&format!("notify: {title} / {body}"));
    app.notification().builder().title(title).body(body).show().map_err(|e| e.to_string())
}

/// 메뉴바 메뉴의 글자를 화면 언어에 맞춘다.
struct MenuLabels(Mutex<Vec<MenuItem<tauri::Wry>>>);
/// 메뉴바 메뉴의 「위젯을 다른 창 위에 띄우기」 체크 항목.
struct TopItem(CheckMenuItem<tauri::Wry>);

/// 메뉴바 메뉴의 「새 판 받기」 항목. 새 판이 있을 때만 메뉴 맨 위에 넣는다.
struct UpdateMenu {
    menu: Menu<tauri::Wry>,
    item: Mutex<Option<MenuItem<tauri::Wry>>>,
    /// 화면 언어에 맞춘 글자 틀. {0} 자리에 판 번호가 들어간다. (새 판 받기, 다시 시작해 업데이트)
    template: Mutex<(String, String)>,
}

/// 자동 업데이트: 받아 두었다가 다시 시작하거나 종료할 때 적용할 새 판.
struct Pending(Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>);

/// 지금 상태(새 판 확인 결과 + 받아 둔 판).
fn update_info(app: &tauri::AppHandle) -> update::Info {
    let mut info = update::info();
    if let Some(p) = app.try_state::<Pending>() {
        if let Ok(slot) = p.0.lock() {
            info.ready = slot.as_ref().map(|(u, _)| u.version.clone());
        }
    }
    info
}

/// 새 판이 있으면 받아 둔다(서명은 설치할 때 공개 키로 확인한다). latest.json이 없는 예전 판에서는 아무것도 하지 않는다.
fn download_update(app: &tauri::AppHandle) {
    let already = app.try_state::<Pending>().and_then(|p| p.0.lock().ok().and_then(|s| s.as_ref().map(|(u, _)| u.version.clone())));
    let res = tauri::async_runtime::block_on(async {
        let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
        let Some(upd) = found else { return Ok(None) };
        if already.as_deref() == Some(upd.version.as_str()) {
            return Ok(None);
        }
        let bytes = upd.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
        Ok::<_, String>(Some((upd, bytes)))
    });
    match res {
        Ok(Some((upd, bytes))) => {
            trace(&format!("새 판 받아 둠: {} ({}바이트)", upd.version, bytes.len()));
            if let Some(p) = app.try_state::<Pending>() {
                if let Ok(mut slot) = p.0.lock() {
                    *slot = Some((upd, bytes));
                }
            }
        }
        Ok(None) => trace("자동 업데이트: 받을 판 없음"),
        Err(e) => trace(&format!("자동 업데이트 받기 실패: {e}")),
    }
}

/// 받아 둔 새 판을 설치한다. restart가 참이면 맥에서 다시 켠다(윈도우는 설치 프로그램이 앱을 닫고 이어 받는다).
/// 받아 둔 판이 없으면 false.
fn install_pending(app: &tauri::AppHandle, restart: bool) -> bool {
    let Some(p) = app.try_state::<Pending>() else { return false };
    let Some((upd, bytes)) = p.0.lock().ok().and_then(|mut s| s.take()) else { return false };
    match upd.install(&bytes) {
        Ok(()) => {
            trace(&format!("새 판 설치: {}", upd.version));
            if restart {
                app.restart();
            }
            true
        }
        Err(e) => {
            trace(&format!("새 판 설치 실패: {e}"));
            false
        }
    }
}

/// 대시보드의 「다시 시작해 업데이트」 단추.
#[tauri::command]
fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    if install_pending(&app, true) {
        Ok(())
    } else {
        Err("받아 둔 새 판이 없어요.".into())
    }
}

#[tauri::command]
fn set_menu_labels(app: tauri::AppHandle, state: tauri::State<MenuLabels>, top_item: tauri::State<TopItem>, show: String, widget: String, quit: String, top: Option<String>, update: Option<String>, ready: Option<String>) {
    if let Ok(items) = state.0.lock() {
        for (item, text) in items.iter().zip([show, widget, quit]) {
            let _ = item.set_text(text);
        }
    }
    if let Some(text) = top {
        let _ = top_item.0.set_text(text);
    }
    if let (Some(text), Some(um)) = (update, app.try_state::<UpdateMenu>()) {
        if let Ok(mut t) = um.template.lock() {
            t.0 = text;
            if let Some(r) = ready {
                t.1 = r;
            }
        }
        apply_update(&app, &update_info(&app));
    }
}

/// 새 판 정보를 메뉴바 메뉴와 대시보드에 반영한다.
fn apply_update(app: &tauri::AppHandle, info: &update::Info) {
    if let Some(um) = app.try_state::<UpdateMenu>() {
        let (get, ready) = um.template.lock().map(|t| t.clone()).unwrap_or_else(|_| ("새 판 {0} 받기".into(), "다시 시작해 {0}로 업데이트".into()));
        // 받아 둔 판이 있으면 「다시 시작해 업데이트」, 아니면 「새 판 받기」(내려받는 페이지 열기).
        let (template, shown) = match &info.ready {
            Some(v) => (ready, Some(v.clone())),
            None => (get, info.newer.clone()),
        };
        if let Ok(mut slot) = um.item.lock() {
            match (&shown, slot.as_ref()) {
                (Some(v), Some(item)) => {
                    let _ = item.set_text(template.replace("{0}", v));
                }
                (Some(v), None) => {
                    if let Ok(item) = MenuItem::with_id(app, "update", template.replace("{0}", v), true, None::<&str>) {
                        if um.menu.insert(&item, 0).is_ok() {
                            *slot = Some(item);
                        }
                    }
                }
                (None, Some(item)) => {
                    let _ = um.menu.remove(item);
                    *slot = None;
                }
                (None, None) => {}
            }
        }
    }
    if let (Some(w), Ok(arg)) = (app.get_webview_window("main"), serde_json::to_string(info)) {
        let _ = w.eval(&format!("window.__twUpdate && window.__twUpdate({arg})"));
    }
}

/// 새 판 알림 상태(대시보드가 처음 열릴 때 읽는다).
#[tauri::command]
fn get_update(app: tauri::AppHandle) -> update::Info {
    update_info(&app)
}

/// 새 판 알림을 켜거나 끈다. 켜면 바로 한 번 확인한다.
#[tauri::command]
fn set_update(app: tauri::AppHandle, on: bool) -> Result<update::Info, String> {
    trace(&format!("set_update: {on}"));
    update::set_on(on)?;
    if !on {
        // 끄면 받아 둔 판도 버린다.
        if let Some(p) = app.try_state::<Pending>() {
            if let Ok(mut s) = p.0.lock() {
                *s = None;
            }
        }
    }
    let info = update_info(&app);
    apply_update(&app, &info);
    if on {
        std::thread::spawn(move || {
            let s = update::check_at(&usage::home(), chrono::Local::now().timestamp());
            if update::info_of(&s, &update::current()).newer.is_some() {
                download_update(&app);
            }
            apply_update(&app, &update_info(&app));
        });
    }
    Ok(info)
}

/// 새 판 릴리스 페이지를 연다(이 저장소의 릴리스 주소만).
#[tauri::command]
fn open_release(url: String) -> Result<(), String> {
    update::open_release(&url)
}

/// 앱을 켜고 20초 뒤부터 한 시간마다, 하루가 지났으면 깃허브에 새 판을 묻는다.
fn start_update_checker(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(20));
        loop {
            let s = update::check_at(&usage::home(), chrono::Local::now().timestamp());
            if update::info_of(&s, &update::current()).newer.is_some() {
                download_update(&app);
            }
            let info = update_info(&app);
            trace(&format!("새 판 확인: {info:?}"));
            apply_update(&app, &info);
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });
}

/// 메뉴바 메뉴의 체크 표시를 실제 위젯 상태에 맞춘다.
fn sync_top_item(app: &tauri::AppHandle) {
    if let Some(item) = app.try_state::<TopItem>() {
        let _ = item.0.set_checked(widget::is_all_top());
    }
}

/// 선택 기능 「정확한 Claude 한도 가져오기」가 켜져 있는지(경고에 동의하고 켰을 때만 참).
#[tauri::command]
fn get_claude_direct() -> bool {
    claude_api::is_on()
}

/// 선택 기능을 켜거나 끈다. 화면은 약관 위험 경고에 동의한 뒤에만 켜기를 부른다.
#[tauri::command]
fn set_claude_direct(on: bool) -> Result<bool, String> {
    trace(&format!("set_claude_direct: {on}"));
    claude_api::set_on(on)
}

/// 바탕화면 위젯이 켜져 있는지.
#[tauri::command]
fn get_widget() -> bool {
    widget::is_on()
}

/// 바탕화면 위젯을 켜거나 끈다.
#[tauri::command]
fn set_widget(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    trace(&format!("set_widget: {on}"));
    widget::set(&app, on)
}

/// 위젯을 하나 더 띄운다.
#[tauri::command]
fn add_widget(app: tauri::AppHandle) -> Result<(), String> {
    trace("add_widget");
    widget::add(&app)
}

/// 이 위젯을 닫고 목록에서 뺀다(위젯 창에서 호출).
#[tauri::command]
fn remove_widget(app: tauri::AppHandle, window: tauri::Window) -> Result<(), String> {
    widget::remove(&app, window.label())
}

/// 이 위젯의 꾸밈(모양, 보여 줄 항목, 서비스, 풍경)을 기억한다.
#[tauri::command]
fn set_widget_opts(window: tauri::Window, opts: serde_json::Value) -> Result<(), String> {
    widget::set_opts(window.label(), opts)
}

/// 이 위젯의 크기를 바꾼다(모양을 바꿨을 때).
#[tauri::command]
fn widget_resize(window: tauri::Window, width: f64, height: f64) {
    if width.is_finite() && height.is_finite() && width >= 100.0 && height >= 40.0 {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
}

/// 이 위젯을 주 화면 가운데로 옮긴다(다른 모니터에 있거나 화면 밖으로 나갔을 때).
/// 모니터마다 배율이 달라도 맞도록 점(논리 좌표) 단위로 계산한다.
#[tauri::command]
fn widget_to_primary(window: tauri::Window) {
    let (Ok(Some(monitor)), Ok(size), Ok(scale)) = (window.primary_monitor(), window.outer_size(), window.scale_factor()) else { return };
    let ms = monitor.scale_factor();
    let (mp, msize) = (monitor.position().to_logical::<f64>(ms), monitor.size().to_logical::<f64>(ms));
    let wsize = size.to_logical::<f64>(scale);
    let x = mp.x + (msize.width - wsize.width) / 2.0;
    let y = mp.y + (msize.height - wsize.height) / 2.0;
    trace(&format!("widget_to_primary: ({x:.0}, {y:.0})"));
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    let _ = window.set_visible_on_all_workspaces(true);
    // 바탕화면에 깔린 위젯이면 옮긴 자리가 보이게 잠깐 다른 창 위로 올린다.
    if let Some(w) = window.app_handle().get_webview_window(window.label()) {
        widget::flash(&w);
    }
}

/// 이 위젯을 다른 창 위에 띄울지(켜면 항상 위, 끄면 바탕화면에 깔림).
#[tauri::command]
fn widget_set_top(window: tauri::WebviewWindow, on: bool) {
    let level = widget::set_level(&window, on);
    trace(&format!("widget_set_top: {on} {level:?}"));
    let _ = window.set_visible_on_all_workspaces(true);
    // 꾸밈 저장은 화면이 따로 하지만, 메뉴바 체크 표시가 바로 맞도록 여기서도 적어 둔다.
    let _ = widget::set_top_of(window.label(), on);
    sync_top_item(window.app_handle());
}

/// 위젯을 오른쪽 클릭했을 때 메뉴를 띄운다.
#[tauri::command]
fn widget_menu(window: tauri::WebviewWindow, spec: serde_json::Value) -> Result<(), String> {
    widget::popup(&window, &spec)
}

/// 테두리 없는 위젯 창을 끌어 옮긴다(화면에서 누르고 있을 때 호출).
#[tauri::command]
fn widget_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

/// 위젯에서 대시보드를 연다.
#[tauri::command]
fn open_dashboard(app: tauri::AppHandle) {
    show_main(&app);
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// 20초마다 모든 창의 화면에 갱신을 알린다(세 번에 한 번은 새로 읽기).
/// 대시보드 창이 숨겨지면 화면 쪽 타이머가 멈춰 메뉴바와 위젯 값이 처음 값에 머물렀다(2026-11-14 측정: 숨김 3분에 1번, 보임 3분에 6번).
fn start_ticker(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut n: u64 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(20));
            n += 1;
            let full = n % 3 == 0;
            for (_, w) in app.webview_windows() {
                let _ = w.eval(&format!("window.__twTick && window.__twTick({full})"));
            }
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "대시보드 열기", true, None::<&str>)?;
            let wid = MenuItem::with_id(app, "widget", "바탕화면 위젯 켜고 끄기", true, None::<&str>)?;
            let top = CheckMenuItem::with_id(app, "widget-top", "위젯을 다른 창 위에 띄우기", true, widget::is_all_top(), None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &wid, &top, &quit])?;
            app.manage(TopItem(top.clone()));
            app.manage(UpdateMenu { menu: menu.clone(), item: Mutex::new(None), template: Mutex::new(("새 판 {0} 받기".into(), "다시 시작해 {0}로 업데이트".into())) });
            app.manage(Pending(Mutex::new(None)));
            app.manage(MenuLabels(Mutex::new(vec![show.clone(), wid.clone(), quit.clone()])));
            let mut tray = TrayIconBuilder::with_id("main")
                .title("DeskDusk")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "widget" => {
                        let _ = widget::set(app, !widget::is_on());
                        sync_top_item(app);
                    }
                    // 체크 항목은 누르면 표시가 스스로 뒤집히므로, 실제 상태를 기준으로 뒤집고 다시 맞춘다.
                    "widget-top" => {
                        let _ = widget::set_all_top(app, !widget::is_all_top());
                        sync_top_item(app);
                    }
                    "quit" => {
                        // 받아 둔 새 판이 있으면 종료할 때 바꿔 둔다(다시 켜지는 않는다).
                        install_pending(app, false);
                        app.exit(0)
                    }
                    "update" => {
                        if !install_pending(app, true) {
                            if let Some(url) = update::info().url {
                                let _ = update::open_release(&url);
                            }
                        }
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            // 로그인 때 자동으로 켜진 경우에는 창을 띄우지 않고 메뉴바에만 올린다.
            if std::env::args().any(|a| a == autostart::HIDDEN_ARG) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            // 처음 실행이면 위젯 하나를 켠 상태로 시작하고, 켜 두었던 바탕화면 위젯을 띄운다.
            if widget::init_first_run() {
                trace("처음 실행: 위젯을 켠 상태로 시작");
            }
            widget::open_all(app.handle());
            start_ticker(app.handle().clone());
            start_update_checker(app.handle().clone());
            apply_update(app.handle(), &update_info(app.handle()));
            // 위젯 메뉴에서 고른 항목은 그 위젯 화면으로 돌려보낸다.
            app.on_menu_event(|app, event| {
                if let Some((label, item)) = widget::menu_target(event.id().as_ref()) {
                    if let Some(w) = app.get_webview_window(label) {
                        let arg = serde_json::to_string(item).unwrap_or_default();
                        let _ = w.eval(&format!("window.__twMenu && window.__twMenu({arg})"));
                    }
                }
            });
            Ok(())
        })
        // 창을 닫아도 메뉴바 요약은 남도록 숨기기만 한다.
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } if !window.label().starts_with(widget::LABEL_PREFIX) => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) if window.label().starts_with(widget::LABEL_PREFIX) => widget::remember(window),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_usage, set_tray_title, get_autostart, set_autostart, notify, set_menu_labels, get_connect_status, set_claude_connect, get_widget, set_widget, widget_drag, open_dashboard, add_widget, remove_widget, set_widget_opts, widget_resize, widget_menu, widget_to_primary, widget_set_top, install_place, get_update, set_update, open_release, get_claude_direct, set_claude_direct, install_update])
        .build(tauri::generate_context!())
        .expect("앱을 시작하지 못했습니다")
        .run(|app, event| {
            // ⌘Q, 로그아웃 등 어떤 길로 끝나든 받아 둔 새 판이 있으면 바꿔 둔다(한 번만 적용된다).
            if let tauri::RunEvent::Exit = event {
                install_pending(app, false);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::place_of;

    #[test]
    fn 디스크_이미지_안에서_실행하면_알아챈다() {
        assert_eq!(place_of("/Volumes/DeskDusk/DeskDusk.app/Contents/MacOS/deskdusk"), "dmg");
        assert_eq!(place_of("/Volumes/DeskDusk 1/DeskDusk.app/Contents/MacOS/deskdusk"), "dmg");
        assert_eq!(place_of("/private/var/folders/x/T/AppTranslocation/ABC/d/DeskDusk.app/Contents/MacOS/deskdusk"), "translocated");
        assert_eq!(place_of("/Applications/DeskDusk.app/Contents/MacOS/deskdusk"), "ok");
        assert_eq!(place_of("/Users/a/Applications/DeskDusk.app/Contents/MacOS/deskdusk"), "ok");
        // 외장 디스크의 폴더 안에 둔 앱(개발 빌드 등)은 디스크 이미지로 보지 않는다.
        assert_eq!(place_of("/Volumes/Work/Builds/x/DeskDusk.app/Contents/MacOS/deskdusk"), "ok");
    }
}
