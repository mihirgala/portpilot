# PortPilot

A small tray app for recognized development web servers, built with Tauri and Vite for Windows, macOS, and Linux. Click the tray icon to open the popup; right-click it for the menu and Quit. On Linux desktops where tray click events are unavailable, use the tray menu's Open PortPilot item.

PortPilot shows only listening TCP processes whose command lines match known development servers, such as CodeIgniter/PHP, Vite, Next.js, Python HTTP server, Uvicorn, Django, and common Go and Rust development launchers. Go and Rust binaries started via `go run` or `cargo run` are recognized by their launcher ancestry; independently launched compiled binaries may be omitted. It intentionally omits unknown and custom servers rather than listing all applications. Detection is based on command-line patterns and is not a guarantee of intent; check the PID and ports in the confirmation before stopping. Stop forcibly terminates the process and its children, which can lose unsaved work. Some processes cannot be inspected or stopped without sufficient permissions.

## Build

Install Node.js, Rust, and the platform dependencies for Tauri 2. Run `npm ci` followed by `npm run tauri build`. GitHub Actions builds Windows, macOS (Intel and Apple Silicon), and Linux artifacts. Windows also has a portable executable at `src-tauri/target/release/portpilot.exe` and requires WebView2 Runtime. Linux needs a desktop with a system tray or AppIndicator support. macOS builds are unsigned and may require local approval to open. For development, run `npm run tauri dev`.

