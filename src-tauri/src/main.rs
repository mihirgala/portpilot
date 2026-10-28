#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use sysinfo::{Pid, ProcessesToUpdate, System};
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use tauri::{Emitter, Manager, PhysicalPosition, menu::{Menu, MenuItem}, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState}};

#[derive(Serialize)]
struct Listener { pid: u32, name: String, started: String, addresses: Vec<String>, launcher: Option<Identity> }
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
struct Identity { pid: u32, started: String }

// Match specific development server commands, never a generic runtime such as node or python.
fn server_name(command: &str) -> Option<&'static str> {
    let cmd = format!(" {} ", command.to_lowercase().replace('\\', "/"));
    let patterns = [
        ("/vite/bin/vite", "Vite"), (" vite ", "Vite"), (" vite.js", "Vite"), ("next-server", "Next.js"), ("next dev", "Next.js"),
        ("react-scripts start", "Create React App"), ("webpack-dev-server", "Webpack"),
        ("webpack serve", "Webpack"), ("nuxt dev", "Nuxt"), ("nuxi dev", "Nuxt"),
        ("astro dev", "Astro"), ("remix dev", "Remix"), ("svelte-kit dev", "SvelteKit"),
        ("http.server", "Python HTTP server"), ("uvicorn", "Uvicorn"), ("hypercorn", "Hypercorn"),
        ("flask run", "Flask"), ("manage.py runserver", "Django"),
        ("rails server", "Rails"), (" spark serve", "CodeIgniter"), ("php -s ", "PHP server"), ("php.exe -s ", "PHP server"), ("artisan serve", "Laravel"), ("symfony server:start", "Symfony"),
        ("cargo watch", "Rust development server"), ("cargo shuttle run", "Shuttle"), ("trunk serve", "Trunk"), ("dioxus serve", "Dioxus"), ("dx serve", "Dioxus"), ("leptos serve", "Leptos"),
        ("air -c", "Go development server"), ("wgo run", "Go development server"), ("gin-bin", "Go development server"), ("hugo server", "Hugo"),
        ("dotnet watch", ".NET development server"), ("dotnet run", ".NET development server"), ("spring-boot:run", "Spring Boot"), ("quarkus:dev", "Quarkus"),
        ("gunicorn", "Gunicorn"), ("daphne", "Daphne"), ("fastapi dev", "FastAPI"), ("streamlit run", "Streamlit"), ("gradio", "Gradio"),
        ("http-server", "http-server"), ("live-server", "live-server"), ("serve-handler", "serve"),
        ("/angular/cli/bin/ng", "Angular"), ("ng serve", "Angular"), ("parcel serve", "Parcel"),
        ("bun --hot", "Bun"), ("deno serve", "Deno"),
    ];
    patterns.into_iter().find_map(|(needle, name)| if cmd.contains(needle) { Some(name) } else { None })
}
fn process_command(process: &sysinfo::Process) -> String {
    process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ")
}

fn is_spark_launcher(args: &[std::ffi::OsString]) -> bool {
    let args: Vec<String> = args.iter().map(|s| s.to_string_lossy().to_lowercase().replace('\\', "/")).collect();
    let Some(exe) = args.first() else { return false; };
    if !matches!(exe.rsplit('/').next(), Some("php" | "php.exe")) { return false; }
    args.windows(2).skip(1).any(|w| w[0].rsplit('/').next() == Some("spark") && w[1] == "serve")
}
fn spark_launcher(system: &System, listener: Pid) -> Option<Identity> {
    let mut current = system.process(listener)?.parent();
    for _ in 0..8 {
        let process = system.process(current?)?;
        if is_spark_launcher(process.cmd()) && process.start_time() != 0 {
            return Some(Identity { pid: process.pid().as_u32(), started: process.start_time().to_string() });
        }
        current = process.parent();
    }
    None
}
// A compiled Go or Rust listener usually has an arbitrary executable name. Recognize it
// only when a development launcher is in its ancestry, rather than all native programs.
fn recognized_process(system: &System, pid: Pid) -> Option<&'static str> {
    let process = system.process(pid)?;
    if let Some(name) = server_name(&process_command(process)) { return Some(name); }
    let mut parent = process.parent();
    for _ in 0..8 {
        let ancestor = system.process(parent?)?;
        let cmd = format!(" {} ", process_command(ancestor).to_lowercase().replace('\\', "/"));
        let words: Vec<&str> = cmd.split_whitespace().collect();
        let exe = words.first().and_then(|s| s.rsplit('/').next()).unwrap_or("").trim_end_matches(".exe");
        if exe == "go" && words.get(1) == Some(&"run") { return Some("Go development server"); }
        if exe == "cargo" && words.get(1) == Some(&"run") { return Some("Rust development server"); }
        if exe == "php" && words.windows(2).any(|w| w[0].ends_with("spark") && w[1] == "serve") { return Some("CodeIgniter"); }
        parent = ancestor.parent();
    }
    None
}
fn snapshot() -> Result<Vec<Listener>, String> {
    let sockets = get_sockets_info(AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6, ProtocolFlags::TCP).map_err(|e| format!("Could not read listening ports: {e}"))?;
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut grouped: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for socket in sockets {
        if let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info {
            if tcp.state == TcpState::Listen {
                for pid in socket.associated_pids { grouped.entry(pid).or_default().push(format!("{}:{}", tcp.local_addr, tcp.local_port)); }
            }
        }
    }
    Ok(grouped.into_iter().filter_map(|(pid, mut addresses)| {
        let process = system.process(Pid::from_u32(pid))?;
        let name = recognized_process(&system, Pid::from_u32(pid))?;
        if process.start_time() == 0 { return None; }
        addresses.sort(); addresses.dedup();
        Some(Listener { pid, name: if spark_launcher(&system, Pid::from_u32(pid)).is_some() { "CodeIgniter".into() } else { name.into() }, started: process.start_time().to_string(), addresses, launcher: spark_launcher(&system, Pid::from_u32(pid)) })
    }).collect())
}
#[tauri::command]
async fn list_listeners() -> Result<Vec<Listener>, String> {
    tauri::async_runtime::spawn_blocking(snapshot).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn stop_listener(pid: u32, started: String, launcher: Option<Identity>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut system = System::new_all(); system.refresh_processes(ProcessesToUpdate::All, true);
        let target = system.process(Pid::from_u32(pid)).ok_or("The server has already exited.")?;
        if target.start_time().to_string() != started || recognized_process(&system, Pid::from_u32(pid)).is_none() || pid == std::process::id() { return Err("The process has changed. Refresh and try again.".into()); }
        if launcher.is_none() && process_command(target).to_lowercase().replace('\\', "/").contains("/system/rewrite.php") { return Err("Could not identify the CodeIgniter launcher safely. Stop it from its terminal instead.".into()); }
        // Confirm the launcher is the same one shown in the confirmation. Never select a
        // terminal or an unrecognized ancestor as the root of the process tree.
        if spark_launcher(&system, Pid::from_u32(pid)) != launcher { return Err("The server launcher changed. Refresh and try again.".into()); }
        let root = launcher.as_ref().map(|p| Pid::from_u32(p.pid)).unwrap_or(Pid::from_u32(pid));
        let mut ids = vec![root]; let mut seen = HashSet::from([root]);
        let mut index = 0;
        while index < ids.len() { let parent = ids[index]; for (id, process) in system.processes() { if process.parent() == Some(parent) && seen.insert(*id) { ids.push(*id); } } index += 1; }
        let identities: Vec<(Pid, u64)> = ids.iter().filter_map(|id| system.process(*id).map(|p| (*id, p.start_time()))).collect();
        // Kill the launcher first so it cannot restart a server as its child exits.
        let root_process = system.process(root).ok_or("The server has already exited.")?;
        let root_process_start = root_process.start_time();
        if !root_process.kill() { return Err("Could not stop the server launcher. It may require additional permissions.".into()); }
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            system.refresh_processes(ProcessesToUpdate::Some(&[root]), true);
            if system.process(root).map(|p| p.start_time()) != Some(root_process_start) { break; }
        }
        if system.process(root).map(|p| p.start_time()) == Some(root_process_start) { return Err("The server launcher did not exit. Try again.".into()); }
        let mut failed = false;
        for (id, start) in identities.into_iter().skip(1).rev() {
            system.refresh_processes(ProcessesToUpdate::Some(&[id]), true);
            if let Some(p) = system.process(id) { if p.start_time() == start && !p.kill() { std::thread::sleep(std::time::Duration::from_millis(100)); system.refresh_processes(ProcessesToUpdate::Some(&[id]), true); if system.process(id).map(|p| p.start_time()) == Some(start) { failed = true; } } }
        }
        if failed { Err("Could not stop every server process. It may require additional permissions.".into()) } else { Ok(()) }
    }).await.map_err(|e| e.to_string())?
}
fn show_popup(app: &tauri::AppHandle, position: Option<PhysicalPosition<i32>>) {
    if let Some(w) = app.get_webview_window("main") {
        if let Some(pos) = position {
            if let Ok(Some(monitor)) = w.monitor_from_point(pos.x as f64, pos.y as f64) {
                let origin = monitor.position(); let size = monitor.size(); let window = w.outer_size().unwrap_or(tauri::PhysicalSize::new(390, 520));
                let x = (pos.x - window.width as i32 / 2).clamp(origin.x, origin.x + size.width as i32 - window.width as i32);
                let y = if pos.y - origin.y < size.height as i32 / 2 { pos.y + 25 } else { pos.y - window.height as i32 - 8 };
                let y = y.clamp(origin.y, origin.y + size.height as i32 - window.height as i32);
                let _ = w.set_position(PhysicalPosition::new(x, y));
            }
        }
        let _ = w.show(); let _ = w.set_focus();
    }
}
fn main() {
    tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| { show_popup(app, None); let _ = app.emit("already-running", ()); })).setup(|app| {
        #[cfg(target_os = "macos")]
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        let show = MenuItem::with_id(app, "show", "Open PortPilot", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&show, &quit])?;
        let mut tray = TrayIconBuilder::new().menu(&menu).show_menu_on_left_click(false).tooltip("PortPilot").on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_popup(app, None), "quit" => app.exit(0), _ => ()
        }).on_tray_icon_event(|tray, event| { if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = event { show_popup(tray.app_handle(), Some(PhysicalPosition::new(position.x as i32, position.y as i32))); } });
        if let Some(icon) = app.default_window_icon() { tray = tray.icon(icon.clone()); }
        tray.build(app)?;
        Ok(())
    }).on_window_event(|window, event| { match event { tauri::WindowEvent::CloseRequested { api, .. } => { api.prevent_close(); let _ = window.hide(); }, tauri::WindowEvent::Focused(false) => { let _ = window.hide(); }, _ => () } })
    .invoke_handler(tauri::generate_handler![list_listeners, stop_listener]).run(tauri::generate_context!()).expect("error running PortPilot");
}
#[cfg(test)]
mod tests { use super::{server_name, is_spark_launcher}; #[test] fn identifies_specific_servers_only() { assert_eq!(server_name("node /project/node_modules/vite/bin/vite.js"), Some("Vite")); assert_eq!(server_name("python -m http.server 8000"), Some("Python HTTP server")); assert_eq!(server_name("node desktop-app.js"), None); assert_eq!(server_name("python worker.py"), None); assert_eq!(server_name("php spark serve"), Some("CodeIgniter")); assert_eq!(server_name("php.exe -S localhost:8080"), Some("PHP server")); assert_eq!(server_name("trunk serve"), Some("Trunk")); assert_eq!(server_name("hugo server"), Some("Hugo")); } #[test] fn recognizes_only_spark_launcher() { let args = |s: &[&str]| s.iter().map(std::ffi::OsString::from).collect::<Vec<_>>(); assert!(is_spark_launcher(&args(&["C:\\Program Files\\PHP\\php.exe", "C:\\my app\\spark", "serve"]))); assert!(!is_spark_launcher(&args(&["cmd.exe", "/c", "php spark serve"]))); assert!(!is_spark_launcher(&args(&["php", "spark", "migrate"]))); } }







#[cfg(all(test, windows))]
mod integration_tests {
    use super::*;
    use std::{fs, process::{Command, Stdio}, time::Duration};
    #[test]
    #[ignore = "requires PHP installed locally"]
    fn stops_spark_launcher_before_server_can_retry() {
        let dir = std::env::temp_dir().join(format!("portpilot-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("spark"), r#"<?php
$port = (int)$argv[2];
for ($i = 0; $i < 10; $i++) {
  passthru('"' . PHP_BINARY . '" -S 127.0.0.1:' . ($port + $i), $status);
  if ($status === 0) break;
}
"#).unwrap();
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port(); drop(socket);
        let mut child = Command::new("php").current_dir(&dir).args(["spark", "serve", &port.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let entry = (0..100).find_map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            snapshot().unwrap().into_iter().find(|e| e.launcher.as_ref().map(|p| p.pid) == Some(child.id()))
        }).expect("PHP listener was not found");
        assert_eq!(entry.name, "CodeIgniter");
        let launcher_pid = child.id();
        tauri::async_runtime::block_on(stop_listener(entry.pid, entry.started, entry.launcher)).unwrap();
        child.wait().unwrap();
        std::thread::sleep(Duration::from_millis(700));
        assert!(!snapshot().unwrap().iter().any(|e| e.launcher.as_ref().map(|p| p.pid) == Some(launcher_pid)));
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}




