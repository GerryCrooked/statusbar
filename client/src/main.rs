//! Statusbar desktop client for Windows.
//!
//! One small process per interactive session. It long-polls the server and shows
//! incidents as a shell AppBar (reserved screen edge) on every monitor.
//! It never moves, resizes or otherwise touches windows of other applications.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("This client runs on Windows only.");
}

#[cfg(windows)]
fn main() {
    win::main()
}

#[cfg(windows)]
mod win {
    use serde::Deserialize;
    use std::{
        collections::{HashMap, HashSet},
        env,
        ffi::c_void,
        fs,
        io::Write,
        mem::{size_of, zeroed},
        path::PathBuf,
        ptr::{null, null_mut},
        sync::{
            atomic::{AtomicBool, AtomicU32, Ordering},
            Mutex, OnceLock,
        },
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, EnumDisplayMonitors,
            FillRect, GetMonitorInfoW, InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER,
            DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HDC, HMONITOR, LOGFONTW, MONITORINFO, PAINTSTRUCT,
            TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{SHAppBarMessage, ABE_BOTTOM, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
                KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, SetWindowPos,
                ShowWindow, TranslateMessage, IDC_ARROW, MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
                SW_HIDE, WM_APP, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_ENDSESSION, WM_PAINT, WM_TIMER, WNDCLASSW,
                WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            },
        },
    };

    /// Folder / window name. Resellers can rebrand at build time:
    /// `STATUSBAR_APP_NAME=Acme cargo build --release`.
    const APP: &str = match option_env!("STATUSBAR_APP_NAME") {
        Some(name) => name,
        None => "Statusbar",
    };

    const UPDATE_MESSAGE: u32 = WM_APP + 12;
    const APPBAR_MESSAGE: u32 = WM_APP + 13;
    const TIMER_ID: usize = 1;
    const ABN_POSCHANGED: WPARAM = 1;
    const ABN_FULLSCREENAPP: WPARAM = 2;

    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        weekday: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDpiAwarenessContext(context: isize) -> i32;
        fn GetDpiForWindow(hwnd: HWND) -> u32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn EnumWindows(callback: Option<unsafe extern "system" fn(HWND, LPARAM) -> i32>, param: LPARAM) -> i32;
        fn GetClassNameW(hwnd: HWND, buffer: *mut u16, max: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: HWND, pid: *mut u32) -> u32;
        fn IsWindow(hwnd: HWND) -> i32;
        fn IsWindowVisible(hwnd: HWND) -> i32;
        fn IsIconic(hwnd: HWND) -> i32;
        fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> i32;
        fn MonitorFromWindow(hwnd: HWND, flags: u32) -> HMONITOR;
        fn WindowFromPoint(point: Point) -> HWND;
        fn GetAncestor(hwnd: HWND, flags: u32) -> HWND;
        fn IsZoomed(hwnd: HWND) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
        fn GetLastError() -> u32;
        fn GetLocalTime(time: *mut SystemTime);
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, buffer: *mut u16, size: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            key: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }

    // ---- data ------------------------------------------------------------------

    #[derive(Clone, Deserialize)]
    struct Config {
        server_url: String,
        client_token: String,
        poll_seconds: Option<u64>,
    }

    #[derive(Clone, Deserialize, Default)]
    #[serde(default)]
    struct Incident {
        id: String,
        title: String,
        message: String,
        severity: String,
        status: String,
    }

    #[derive(Clone, Deserialize)]
    #[serde(default)]
    struct Labels {
        error: String,
        warning: String,
        info: String,
        resolved: String,
    }
    impl Default for Labels {
        fn default() -> Self {
            Self {
                error: "Outage".into(),
                warning: "Degraded".into(),
                info: "Notice".into(),
                resolved: "Resolved".into(),
            }
        }
    }

    #[derive(Clone, Deserialize)]
    #[serde(default)]
    struct Display {
        position: String,
        all_monitors: bool,
        height_px: i32,
        initial_height_px: i32,
        initial_seconds: u64,
        over_fullscreen: bool,
        /// Full-screen Remote Desktop: "overlay", "windowed" or "hide".
        rdp_mode: String,
        labels: Labels,
    }
    impl Default for Display {
        fn default() -> Self {
            Self {
                position: "top".into(),
                all_monitors: true,
                height_px: 44,
                initial_height_px: 88,
                initial_seconds: 5,
                over_fullscreen: true,
                rdp_mode: "overlay".into(),
                labels: Labels::default(),
            }
        }
    }
    impl Display {
        fn normalized(mut self) -> Self {
            self.height_px = self.height_px.clamp(24, 160);
            self.initial_height_px = self.initial_height_px.clamp(self.height_px, 300);
            self.initial_seconds = self.initial_seconds.min(60);
            self
        }
    }

    #[derive(Deserialize)]
    struct ServerState {
        #[serde(default)]
        rev: Option<u64>,
        #[serde(default)]
        incidents: Vec<Incident>,
        #[serde(default)]
        display: Display,
    }

    #[derive(Clone, Copy)]
    struct Bar {
        hwnd: usize,
        bounds: RECT,
        primary: bool,
        registered: bool,
        height: i32,
        at_top: bool,
        /// A Remote Desktop session fills this monitor and shows its own bar; ours stays away.
        suppressed: bool,
        /// Consecutive checks that disagreed with `suppressed` (debounce).
        flips: u8,
    }

    struct Ui {
        controller: usize,
        incidents: Vec<Incident>,
        index: usize,
        display: Display,
        bars: Vec<Bar>,
        emphasized_until: Option<Instant>,
        last_rotation: Instant,
    }

    static UI: OnceLock<Mutex<Ui>> = OnceLock::new();
    static PENDING: OnceLock<Mutex<Option<ServerState>>> = OnceLock::new();
    /// Set while we talk to the shell, so our own position changes do not re-enter `layout`.
    static IN_LAYOUT: AtomicBool = AtomicBool::new(false);
    /// "TaskbarCreated": broadcast when Explorer restarts and all appbar registrations are lost.
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

    fn ui() -> std::sync::MutexGuard<'static, Ui> {
        UI.get().expect("ui").lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    fn topmost() -> HWND {
        -1isize as HWND
    }
    fn bottom() -> HWND {
        1isize as HWND
    }

    // ---- logging ---------------------------------------------------------------

    /// Appends one line to %LOCALAPPDATA%\<APP>\client.log. Never logs the token.
    fn log(message: &str) {
        let Some(base) = env::var_os("LOCALAPPDATA") else { return };
        let dir = PathBuf::from(base).join(APP);
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("client.log");
        if fs::metadata(&path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
            let _ = fs::remove_file(&path);
        }
        let mut now: SystemTime = unsafe { zeroed() };
        unsafe { GetLocalTime(&mut now) };
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(
                file,
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
                now.year, now.month, now.day, now.hour, now.minute, now.second, message
            );
        }
    }

    // ---- configuration -----------------------------------------------------------

    fn config_path() -> PathBuf {
        let base = env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        let machine = base.join(APP).join("client.json");
        if machine.exists() {
            machine
        } else {
            env::current_exe().unwrap_or_default().with_file_name("client.json")
        }
    }

    /// Machine-wide value under HKLM\Software\<APP>, written by the MSI installer.
    fn registry_value(name: &str) -> Option<String> {
        let key = wide(&format!("Software\\{APP}"));
        let value = wide(name);
        let mut buffer = [0u16; 1024];
        let mut size = (buffer.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                0x8000_0002u32 as i32 as isize, // HKEY_LOCAL_MACHINE
                key.as_ptr(),
                value.as_ptr(),
                2, // RRF_RT_REG_SZ
                null_mut(),
                buffer.as_mut_ptr() as *mut c_void,
                &mut size,
            )
        };
        if status != 0 {
            return None;
        }
        let chars = (size as usize / 2).min(buffer.len());
        let text = String::from_utf16_lossy(&buffer[..chars]);
        let text = text.trim_matches(char::from(0)).trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    fn file_config() -> Option<Config> {
        let path = config_path();
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) => {
                log(&format!("no registry configuration and cannot read {}: {e}", path.display()));
                return None;
            }
        };
        // Windows PowerShell 5.1 writes a UTF-8 BOM; accept it.
        match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                log(&format!("invalid {}: {e}", path.display()));
                None
            }
        }
    }

    /// Registry (MSI installation) first, then client.json (script installation, testing).
    fn load_config() -> Option<Config> {
        let cfg = match (registry_value("ServerUrl"), registry_value("ClientToken")) {
            (Some(server_url), Some(client_token)) => Config { server_url, client_token, poll_seconds: None },
            _ => file_config()?,
        };
        let url = cfg.server_url.trim();
        let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
        if !url.starts_with("https://") && !local {
            log("server_url must start with https://");
            return None;
        }
        Some(cfg)
    }

    // ---- network ---------------------------------------------------------------

    fn worker(controller: usize, cfg: Config) {
        let user = env::var("USERNAME").unwrap_or_default();
        let computer = env::var("COMPUTERNAME").unwrap_or_default();
        let url = format!("{}/api/client/state", cfg.server_url.trim().trim_end_matches('/'));
        let agent = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(45))
            .user_agent(concat!("statusbar-client/", env!("CARGO_PKG_VERSION")))
            .build()
        {
            Ok(agent) => agent,
            Err(e) => {
                log(&format!("http client: {e}"));
                return;
            }
        };
        let mut rev: Option<u64> = None;
        let mut failures: u64 = 0;
        loop {
            let mut request = agent
                .get(&url)
                .bearer_auth(cfg.client_token.trim())
                .query(&[("user", user.as_str()), ("computer", computer.as_str())]);
            if let Some(current) = rev {
                // Long-poll: the server answers as soon as something changes.
                request = request.query(&[("rev", current.to_string()), ("wait", "25".to_string())]);
            }
            let result = request.send().and_then(|r| r.error_for_status()).and_then(|r| r.json::<ServerState>());
            match result {
                Ok(state) => {
                    if failures > 0 {
                        log("connection restored");
                    }
                    failures = 0;
                    let long_poll = state.rev.is_some();
                    rev = state.rev;
                    if let Ok(mut pending) = PENDING.get().expect("pending").lock() {
                        *pending = Some(state);
                    }
                    unsafe { PostMessageW(controller as HWND, UPDATE_MESSAGE, 0, 0) };
                    // Servers without long-poll support are polled at a fixed interval.
                    thread::sleep(if long_poll {
                        Duration::from_millis(200)
                    } else {
                        Duration::from_secs(cfg.poll_seconds.unwrap_or(10).clamp(5, 300))
                    });
                }
                Err(e) => {
                    // Offline: keep showing the last known state; never invent an all-clear.
                    failures += 1;
                    if failures == 1 || failures % 30 == 0 {
                        log(&format!("server unreachable ({failures}x): {e}"));
                    }
                    rev = None;
                    thread::sleep(Duration::from_secs((failures * 5).min(60)));
                }
            }
        }
    }

    // ---- bars ------------------------------------------------------------------

    unsafe extern "system" fn enum_monitor(monitor: HMONITOR, _hdc: HDC, _rect: *mut RECT, param: LPARAM) -> i32 {
        let list = &mut *(param as *mut Vec<(RECT, bool)>);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            list.push((info.rcMonitor, (info.dwFlags & 1) != 0));
        }
        1
    }

    unsafe fn appbar_data(hwnd: HWND, at_top: bool) -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            uCallbackMessage: APPBAR_MESSAGE,
            uEdge: if at_top { ABE_TOP } else { ABE_BOTTOM },
            rc: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            lParam: 0,
        }
    }

    unsafe fn remove_bars() {
        let bars: Vec<Bar> = std::mem::take(&mut ui().bars);
        for bar in bars {
            let hwnd = bar.hwnd as HWND;
            if bar.registered {
                let mut data = appbar_data(hwnd, bar.at_top);
                SHAppBarMessage(ABM_REMOVE, &mut data);
            }
            DestroyWindow(hwnd);
        }
    }

    /// (Re)creates one hidden bar window per monitor. Called at start and on display changes.
    unsafe fn build_bars() {
        remove_bars();
        let mut monitors: Vec<(RECT, bool)> = Vec::new();
        EnumDisplayMonitors(null_mut(), null(), Some(enum_monitor), &mut monitors as *mut _ as LPARAM);
        let class = wide(APP);
        let instance = GetModuleHandleW(null());
        let mut bars = Vec::new();
        for (bounds, primary) in monitors {
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                bounds.left,
                bounds.top,
                100,
                44,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if !hwnd.is_null() {
                bars.push(Bar {
                    hwnd: hwnd as usize,
                    bounds,
                    primary,
                    registered: false,
                    height: 0,
                    at_top: false,
                    suppressed: false,
                    flips: 0,
                });
            }
        }
        log(&format!("{} monitor(s)", bars.len()));
        ui().bars = bars;
        layout(true);
    }

    unsafe fn layout(force: bool) {
        if IN_LAYOUT.swap(true, Ordering::SeqCst) {
            return;
        }
        layout_inner(force);
        IN_LAYOUT.store(false, Ordering::SeqCst);
    }

    unsafe fn layout_inner(force: bool) {
        let (bars, show, display, big) = {
            let ui = ui();
            let big = ui.emphasized_until.is_some_and(|until| Instant::now() < until);
            (ui.bars.clone(), !ui.incidents.is_empty(), ui.display.clone(), big)
        };
        let at_top = display.position != "bottom";
        let logical = if big { display.initial_height_px } else { display.height_px };
        let mut updates = Vec::new();
        for bar in bars {
            let hwnd = bar.hwnd as HWND;
            if !(show && !bar.suppressed && (display.all_monitors || bar.primary)) {
                if bar.registered {
                    let mut data = appbar_data(hwnd, bar.at_top);
                    SHAppBarMessage(ABM_REMOVE, &mut data);
                }
                ShowWindow(hwnd, SW_HIDE);
                updates.push((bar.hwnd, false, 0, at_top));
                continue;
            }
            // Per-monitor DPI: the configured height is in 96-dpi pixels.
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let height = logical * dpi / 96;
            let mut registered = bar.registered;
            if registered && bar.at_top != at_top {
                let mut old = appbar_data(hwnd, bar.at_top);
                SHAppBarMessage(ABM_REMOVE, &mut old);
                registered = false;
            }
            if !registered {
                let mut data = appbar_data(hwnd, at_top);
                registered = SHAppBarMessage(ABM_NEW, &mut data) != 0;
            }
            if registered && (force || !bar.registered || bar.height != height || bar.at_top != at_top) {
                let mut data = appbar_data(hwnd, at_top);
                data.rc = bar.bounds;
                SHAppBarMessage(ABM_QUERYPOS, &mut data);
                if at_top {
                    data.rc.bottom = data.rc.top + height;
                } else {
                    data.rc.top = data.rc.bottom - height;
                }
                SHAppBarMessage(ABM_SETPOS, &mut data);
                SetWindowPos(
                    hwnd,
                    topmost(),
                    data.rc.left,
                    data.rc.top,
                    data.rc.right - data.rc.left,
                    data.rc.bottom - data.rc.top,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                InvalidateRect(hwnd, null(), 0);
            }
            updates.push((bar.hwnd, registered, height, at_top));
        }
        let mut ui = ui();
        for (hwnd, registered, height, at_top) in updates {
            if let Some(bar) = ui.bars.iter_mut().find(|b| b.hwnd == hwnd) {
                bar.registered = registered;
                bar.height = height;
                bar.at_top = at_top;
            }
        }
    }

    unsafe fn redraw() {
        let bars = ui().bars.clone();
        for bar in bars {
            InvalidateRect(bar.hwnd as HWND, null(), 0);
        }
    }

    /// Z-order only, for our own windows: keeps the bar readable above full-screen
    /// windows such as a remote-desktop session, without touching that window.
    unsafe fn raise(order: HWND) {
        let bars = ui().bars.clone();
        for bar in bars.iter().filter(|b| b.registered) {
            SetWindowPos(bar.hwnd as HWND, order, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    // ---- optional: remote-desktop windows -------------------------------------------
    //
    // Off by default. When the admin enables it, a classic Remote Desktop client
    // (mstsc.exe) that is in full screen on exactly one monitor carrying a bar is asked
    // to leave full screen and is fitted into the free area below/above the bar, so the
    // session is not covered. When the last message is gone it is sent back to full
    // screen, unless the user has moved it meanwhile. Sessions spanning several monitors
    // and all other applications are never touched.

    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_RESTORE: WPARAM = 0xF120;
    const SC_MAXIMIZE: WPARAM = 0xF030;

    struct Moved {
        /// Rectangle we gave the window; None while we wait for it to leave full screen.
        applied: Option<RECT>,
        attempts: u8,
    }
    static MOVED: OnceLock<Mutex<HashMap<usize, Moved>>> = OnceLock::new();
    /// Diagnostics: last reported option state and the windows already described in the log.
    static RDP_STATE: Mutex<Option<bool>> = Mutex::new(None);
    static RDP_SEEN: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

    unsafe fn class_name(hwnd: HWND) -> String {
        let mut class = [0u16; 128];
        let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&class[..len])
    }

    fn same_rect(a: &RECT, b: &RECT) -> bool {
        a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
    }
    fn show_rect(r: &RECT) -> String {
        format!("{},{} {}x{}", r.left, r.top, r.right - r.left, r.bottom - r.top)
    }

    unsafe extern "system" fn collect_window(hwnd: HWND, param: LPARAM) -> i32 {
        let list = &mut *(param as *mut Vec<usize>);
        if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
            list.push(hwnd as usize);
        }
        1
    }

    /// True only for the main window of the classic Remote Desktop client.
    unsafe fn is_remote_desktop(hwnd: HWND) -> bool {
        if class_name(hwnd) != "TscShellContainerClass" {
            return false;
        }
        let mut pid = 0u32;
        if GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid == 0 {
            return false;
        }
        let process = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if process.is_null() {
            return false;
        }
        let mut path = [0u16; 1024];
        let mut size = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0;
        CloseHandle(process);
        ok && String::from_utf16_lossy(&path[..size as usize])
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("mstsc.exe"))
    }

    unsafe fn remote_desktop_windows() {
        let (enabled, monitors) = {
            let ui = ui();
            let enabled = ui.display.rdp_mode == "windowed" && !ui.incidents.is_empty();
            (enabled, ui.bars.iter().filter(|b| b.registered).map(|b| b.bounds).collect::<Vec<_>>())
        };
        {
            let mut last = RDP_STATE.lock().unwrap_or_else(|p| p.into_inner());
            if *last != Some(enabled) {
                *last = Some(enabled);
                let option = ui().display.rdp_mode == "windowed";
                log(&format!(
                    "remote desktop: option {}, {} bar(s) active -> {}",
                    if option { "on" } else { "off" },
                    monitors.len(),
                    if enabled { "watching" } else { "idle" }
                ));
            }
        }
        let mut moved = MOVED.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|p| p.into_inner());
        if !enabled {
            for (id, entry) in moved.drain() {
                let hwnd = id as HWND;
                let mut now: RECT = zeroed();
                let Some(applied) = entry.applied else { continue };
                if IsWindow(hwnd) != 0 && GetWindowRect(hwnd, &mut now) != 0 && same_rect(&now, &applied) {
                    log("remote desktop: returning window to full screen");
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_MAXIMIZE, 0);
                }
            }
            return;
        }
        let mut windows: Vec<usize> = Vec::new();
        EnumWindows(Some(collect_window), &mut windows as *mut _ as LPARAM);
        for id in windows {
            let hwnd = id as HWND;
            let candidate = moved.contains_key(&id) || is_remote_desktop(hwnd);
            {
                // Describe every Remote Desktop window once, also the ones we leave alone.
                let class = class_name(hwnd);
                let mut seen =
                    RDP_SEEN.get_or_init(|| Mutex::new(HashSet::new())).lock().unwrap_or_else(|p| p.into_inner());
                if class.starts_with("Tsc") && seen.insert(id) {
                    let mut r: RECT = zeroed();
                    GetWindowRect(hwnd, &mut r);
                    let mut mi: MONITORINFO = zeroed();
                    mi.cbSize = size_of::<MONITORINFO>() as u32;
                    let on = MonitorFromWindow(hwnd, 2);
                    let monitor = if !on.is_null() && GetMonitorInfoW(on, &mut mi) != 0 {
                        show_rect(&mi.rcMonitor)
                    } else {
                        "?".into()
                    };
                    log(&format!(
                        "remote desktop: window class={class} mstsc={candidate} at {} on monitor {monitor}",
                        show_rect(&r)
                    ));
                }
            }
            if !candidate {
                continue;
            }
            let mut rect: RECT = zeroed();
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            let monitor = MonitorFromWindow(hwnd, 2); // MONITOR_DEFAULTTONEAREST
            if GetWindowRect(hwnd, &mut rect) == 0 || monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
                continue;
            }
            if !monitors.iter().any(|m| same_rect(m, &info.rcMonitor)) {
                continue;
            }
            if same_rect(&rect, &info.rcMonitor) {
                // Full screen on exactly this monitor: ask it to become a normal window.
                let entry = moved.entry(id).or_insert(Moved { applied: None, attempts: 0 });
                entry.applied = None;
                if entry.attempts < 3 {
                    entry.attempts += 1;
                    log(&format!(
                        "remote desktop: full screen at {} - asking it to leave (try {})",
                        show_rect(&rect),
                        entry.attempts
                    ));
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_RESTORE, 0);
                } else if entry.attempts == 3 {
                    entry.attempts = 4;
                    log("remote desktop: window stayed in full screen, leaving it alone");
                }
                continue;
            }
            let Some(entry) = moved.get_mut(&id) else { continue };
            let work = info.rcWork;
            let ours = entry.applied.is_some_and(|applied| same_rect(&applied, &rect));
            let fits = entry.applied.is_some() && rect.top >= work.top && rect.bottom <= work.bottom;
            if entry.applied.is_none() || (ours && !fits) {
                // Just left full screen, or the bar changed height: use the free area.
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    work.left,
                    work.top,
                    work.right - work.left,
                    work.bottom - work.top,
                    0x0014,
                ); // NOZORDER | NOACTIVATE
                let mut after: RECT = zeroed();
                GetWindowRect(hwnd, &mut after);
                log(&format!("remote desktop: fitted window into {} (now {})", show_rect(&work), show_rect(&after)));
                entry.applied = Some(after);
                entry.attempts = 0;
            }
        }
        moved.retain(|id, _| IsWindow(*id as HWND) != 0);
    }

    /// Mode "hide": a Remote Desktop window that is in front and fills a monitor (full screen,
    /// maximised, or one monitor of a multi-monitor session) hides our bar on that monitor,
    /// because the client installed on the session host shows the message inside the session.
    /// Read-only towards the other window. Changes need two checks in a row (no flicker).
    unsafe fn remote_desktop_monitors() {
        let hide = ui().display.rdp_mode == "hide";
        let bars = ui().bars.clone();
        let mut changes: Vec<(usize, bool)> = Vec::new();
        let mut counts: Vec<(usize, u8)> = Vec::new();
        for bar in &bars {
            let covered = hide && {
                let b = bar.bounds;
                let centre = Point { x: b.left + (b.right - b.left) / 2, y: b.top + (b.bottom - b.top) / 2 };
                let front = GetAncestor(WindowFromPoint(centre), 2); // GA_ROOT
                let mut rect: RECT = zeroed();
                !front.is_null()
                    && is_remote_desktop(front)
                    && GetWindowRect(front, &mut rect) != 0
                    && (IsZoomed(front) != 0
                        || (rect.left <= b.left
                            && rect.top <= b.top
                            && rect.right >= b.right
                            && rect.bottom >= b.bottom))
            };
            if covered == bar.suppressed {
                if bar.flips != 0 {
                    counts.push((bar.hwnd, 0));
                }
            } else if bar.flips >= 1 || !hide {
                changes.push((bar.hwnd, covered));
                log(&format!(
                    "remote desktop: {} bar on monitor {}",
                    if covered { "hiding" } else { "showing" },
                    show_rect(&bar.bounds)
                ));
            } else {
                counts.push((bar.hwnd, bar.flips + 1));
            }
        }
        if changes.is_empty() && counts.is_empty() {
            return;
        }
        {
            let mut ui = ui();
            for bar in ui.bars.iter_mut() {
                if let Some((_, value)) = changes.iter().find(|(h, _)| *h == bar.hwnd) {
                    bar.suppressed = *value;
                    bar.flips = 0;
                } else if let Some((_, n)) = counts.iter().find(|(h, _)| *h == bar.hwnd) {
                    bar.flips = *n;
                }
            }
        }
        if !changes.is_empty() {
            layout(true);
        }
    }

    unsafe fn apply_state() {
        let Some(next) = PENDING.get().expect("pending").lock().ok().and_then(|mut p| p.take()) else { return };
        let display = next.display.normalized();
        {
            let mut ui = ui();
            let known: HashSet<&str> =
                ui.incidents.iter().filter(|i| i.status == "active").map(|i| i.id.as_str()).collect();
            let fresh = next.incidents.iter().any(|i| i.status == "active" && !known.contains(i.id.as_str()));
            drop(known);
            if fresh && display.initial_seconds > 0 {
                ui.emphasized_until = Some(Instant::now() + Duration::from_secs(display.initial_seconds));
                ui.index = 0;
            }
            if next.incidents.is_empty() {
                ui.emphasized_until = None;
            }
            if ui.index >= next.incidents.len() {
                ui.index = 0;
            }
            ui.incidents = next.incidents;
            ui.display = display;
        }
        layout(false);
        redraw();
    }

    unsafe fn tick() {
        let (relayout, rotate, keep_on_top) = {
            let mut ui = ui();
            let mut relayout = false;
            let mut rotate = false;
            if ui.emphasized_until.is_some_and(|until| Instant::now() >= until) {
                ui.emphasized_until = None;
                relayout = true;
            }
            if ui.last_rotation.elapsed() >= Duration::from_secs(7) {
                if ui.incidents.len() > 1 {
                    ui.index = (ui.index + 1) % ui.incidents.len();
                    rotate = true;
                }
                ui.last_rotation = Instant::now();
            }
            (relayout, rotate, ui.display.over_fullscreen && !ui.incidents.is_empty())
        };
        // Cheap when nothing changed; also re-registers bars after a display or shell change.
        let _ = relayout;
        layout(false);
        if rotate {
            redraw();
        }
        if keep_on_top {
            raise(topmost());
        }
        remote_desktop_windows();
        remote_desktop_monitors();
    }

    unsafe fn paint(hwnd: HWND) {
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.is_null() {
            return;
        }
        let mut rect: RECT = zeroed();
        GetClientRect(hwnd, &mut rect);
        let (incident, labels) = {
            let ui = ui();
            (ui.incidents.get(ui.index).cloned(), ui.display.labels.clone())
        };
        if let Some(i) = incident {
            // COLORREF is 0x00BBGGRR.
            let (color, label) = if i.status == "resolved" {
                (0x004F7D2C, labels.resolved)
            } else if i.severity == "warning" {
                (0x00048AC9, labels.warning)
            } else if i.severity == "info" {
                (0x00B36F2F, labels.info)
            } else {
                (0x002C36C0, labels.error)
            };
            let brush = CreateSolidBrush(color);
            FillRect(hdc, &rect, brush);
            DeleteObject(brush);
            SetTextColor(hdc, 0x00FFFFFF);
            SetBkMode(hdc, TRANSPARENT as i32);

            let mut line = String::new();
            if !label.trim().is_empty() {
                line.push_str(label.trim());
                line.push_str(": ");
            }
            line.push_str(i.title.trim());
            let message: String = i.message.split_whitespace().collect::<Vec<_>>().join(" ");
            if !message.is_empty() && message != i.title.trim() {
                line.push_str("  \u{2013}  ");
                line.push_str(&message);
            }
            let text = wide(&line);

            let height = rect.bottom - rect.top;
            let mut font: LOGFONTW = zeroed();
            font.lfHeight = -(height * 2 / 5).clamp(12, 60);
            font.lfWeight = 600;
            font.lfQuality = 5; // CLEARTYPE_QUALITY
            for (dst, src) in font.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
                *dst = src;
            }
            let hfont = CreateFontIndirectW(&font);
            let previous = if hfont.is_null() { null_mut() } else { SelectObject(hdc, hfont) };
            let pad = height / 3;
            rect.left += pad;
            rect.right -= pad;
            DrawTextW(
                hdc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
            if !hfont.is_null() {
                SelectObject(hdc, previous);
                DeleteObject(hfont);
            }
        }
        EndPaint(hwnd, &ps);
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        let is_controller = UI.get().is_some_and(|_| ui().controller == hwnd as usize);
        match msg {
            UPDATE_MESSAGE if is_controller => {
                apply_state();
                0
            }
            WM_TIMER if is_controller => {
                tick();
                0
            }
            WM_DISPLAYCHANGE if is_controller => {
                // Monitor added/removed or resolution changed: start from a clean slate.
                build_bars();
                0
            }
            _ if is_controller && msg != 0 && msg == TASKBAR_CREATED.load(Ordering::Relaxed) => {
                log("shell restarted, registering bars again");
                build_bars();
                0
            }
            APPBAR_MESSAGE => {
                if wp == ABN_POSCHANGED {
                    layout(true);
                } else if wp == ABN_FULLSCREENAPP && !ui().display.over_fullscreen {
                    // Classic appbar etiquette when the admin prefers not to cover full-screen apps.
                    raise(if lp != 0 { bottom() } else { topmost() });
                }
                0
            }
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_ENDSESSION if is_controller => {
                if wp != 0 {
                    remove_bars();
                }
                0
            }
            WM_CLOSE if is_controller => {
                remove_bars();
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY if is_controller => {
                KillTimer(hwnd, TIMER_ID);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    pub fn main() {
        unsafe {
            // One instance per session.
            let name = wide(&format!("Local\\{APP}Client"));
            let mutex = CreateMutexW(null(), 0, name.as_ptr());
            if mutex.is_null() || GetLastError() == 183 {
                return;
            }
            // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
            SetProcessDpiAwarenessContext(-4);
        }
        log(&format!("{APP} client {} starting", env!("CARGO_PKG_VERSION")));
        let Some(cfg) = load_config() else { return };
        let _ = PENDING.set(Mutex::new(None));
        unsafe {
            let class = wide(APP);
            let instance = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            };
            if RegisterClassW(&wc) == 0 {
                log("RegisterClassW failed");
                return;
            }
            // Hidden top-level window: owns the timer and receives display-change broadcasts.
            let controller = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if controller.is_null() {
                log("CreateWindowExW failed");
                return;
            }
            let _ = UI.set(Mutex::new(Ui {
                controller: controller as usize,
                incidents: Vec::new(),
                index: 0,
                display: Display::default(),
                bars: Vec::new(),
                emphasized_until: None,
                last_rotation: Instant::now(),
            }));
            let taskbar_created = wide("TaskbarCreated");
            TASKBAR_CREATED.store(RegisterWindowMessageW(taskbar_created.as_ptr()), Ordering::Relaxed);
            build_bars();
            let handle = controller as usize;
            thread::spawn(move || worker(handle, cfg));
            SetTimer(controller, TIMER_ID, 1000, None);
            let mut message: MSG = zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            remove_bars();
        }
    }
}
