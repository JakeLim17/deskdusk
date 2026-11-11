#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 창 없이 값만 확인하는 점검용 옵션
    if std::env::args().any(|a| a == "--dump") {
        let t = std::time::Instant::now();
        let first = deskdusk_lib::usage::collect();
        let first_ms = t.elapsed().as_millis();
        let t = std::time::Instant::now();
        let _ = deskdusk_lib::usage::collect();
        eprintln!("첫 읽기 {} ms, 다시 읽기 {} ms", first_ms, t.elapsed().as_millis());
        println!("{}", serde_json::to_string_pretty(&first).unwrap());
        return;
    }
    // 연결 상태만 확인하는 점검용 옵션(읽기만 한다)
    if std::env::args().any(|a| a == "--connect-status") {
        println!("{}", serde_json::to_string_pretty(&deskdusk_lib::connect::status()).unwrap());
        return;
    }
    // 점검용: 선택 기능이 켜져 있을 때 서버 응답의 모양을 출력한다(로그인 정보는 출력하지 않는다)
    if std::env::args().any(|a| a == "--claude-usage-shape") {
        if deskdusk_lib::claude_api::is_on() {
            println!("{}", deskdusk_lib::claude_api::debug_shape());
        } else {
            println!("선택 기능이 꺼져 있어요.");
        }
        return;
    }
    // Claude 한도 연결을 명령줄에서 하는 옵션(설정 파일은 백업 후 상태줄 항목만 바꾼다)
    if std::env::args().any(|a| a == "--connect-claude") {
        match deskdusk_lib::connect::connect() {
            Ok(st) => println!("Claude 한도 연결: {}", st.claude_limits),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // 로그인할 때 자동 실행을 명령줄에서 확인·변경하는 옵션: --autostart on|off|status
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--autostart") {
        let result = match args.get(i + 1).map(String::as_str) {
            Some("on") => deskdusk_lib::autostart::set(true),
            Some("off") => deskdusk_lib::autostart::set(false),
            _ => Ok(deskdusk_lib::autostart::is_enabled()),
        };
        match result {
            Ok(on) => println!("자동 실행: {}", if on { "켜짐" } else { "꺼짐" }),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    deskdusk_lib::run()
}
