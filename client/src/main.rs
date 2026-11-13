#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(not(windows))]
fn main() { eprintln!("This desktop client is Windows-only."); }

#[cfg(windows)]
mod win {
use std::{collections::HashSet, env, fs, mem::{size_of, zeroed}, path::PathBuf, ptr::{null, null_mut}, sync::{Mutex, OnceLock}, thread, time::{Duration, Instant}};
use serde::Deserialize;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawTextW,
        EndPaint, EnumDisplayMonitors, FillRect, GetMonitorInfoW, HDC, HMONITOR,
        InvalidateRect, LOGFONTW, MONITORINFO,
        PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, DT_CENTER,
        DT_SINGLELINE, DT_VCENTER, TRANSPARENT,
    },
    System::{LibraryLoader::GetModuleHandleW, WindowsProgramming::{GetComputerNameW, GetUserNameW}},
    UI::{
        Shell::{APPBARDATA, ABM_NEW, ABM_REMOVE, ABM_QUERYPOS, ABM_SETPOS, ABE_BOTTOM, ABE_TOP, SHAppBarMessage},
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
            GetMessageW, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage,
            RegisterClassW, SetTimer, SetWindowPos, ShowWindow, TranslateMessage,
            WNDCLASSW, MSG, IDC_ARROW, SW_HIDE,
            SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
            WM_APP, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_PAINT,
            WM_SETTINGCHANGE, WM_TIMER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

// Experimental MSTSC compatibility. Only same-session top-level windows whose
// executable filename is precisely mstsc.exe are eligible. This never modifies
// other desktop windows and is disabled unless the server explicitly enables it.
mod rdp {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::ffi::c_void;
    type RawHandle = *mut c_void;
    #[link(name="user32")]
    unsafe extern "system" {
        #[link_name="EnumWindows"] fn enum_windows(cb:Option<unsafe extern "system" fn(HWND,LPARAM)->i32>, lp:LPARAM)->i32;
        #[link_name="GetWindowThreadProcessId"] fn window_pid(hwnd:HWND, pid:*mut u32)->u32;
        #[link_name="IsWindowVisible"] fn visible(hwnd:HWND)->i32;
        #[link_name="IsWindow"] fn is_window(hwnd:HWND)->i32;
        #[link_name="IsZoomed"] fn zoomed(hwnd:HWND)->i32;
        #[link_name="IsIconic"] fn iconic(hwnd:HWND)->i32;
        #[link_name="GetWindowRect"] fn window_rect(hwnd:HWND,rect:*mut RECT)->i32;
        #[link_name="MonitorFromWindow"] fn monitor_from_window(hwnd:HWND, flags:u32)->HMONITOR;
    }
    #[link(name="kernel32")]
    unsafe extern "system" {
        #[link_name="OpenProcess"] fn open_process(rights:u32,inherit:i32,pid:u32)->RawHandle;
        #[link_name="QueryFullProcessImageNameW"] fn process_image(handle:RawHandle,flags:u32,buf:*mut u16,length:*mut u32)->i32;
        #[link_name="CloseHandle"] fn close_handle(handle:RawHandle)->i32;
    }
    #[derive(Clone,Copy)]
    struct Adjusted { original:RECT, applied:RECT }
    static ADJUSTED:OnceLock<Mutex<HashMap<usize,Adjusted>>>=OnceLock::new();
    unsafe extern "system" fn collect(hwnd:HWND, p:LPARAM)->i32 {
        let items=&mut *(p as *mut Vec<HWND>);
        if visible(hwnd)!=0 && iconic(hwnd)==0 {items.push(hwnd)}
        1
    }
    unsafe fn mstsc(hwnd:HWND)->bool {
        let mut pid=0u32;
        if window_pid(hwnd,&mut pid)==0 || pid==0 {return false}
        let handle=open_process(0x1000,0,pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if handle.is_null(){return false}
        let mut buf=[0u16;1024];let mut len=buf.len() as u32;
        let ok=process_image(handle,0,buf.as_mut_ptr(),&mut len)!=0;
        close_handle(handle);
        if !ok {return false}
        let path=String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\','/']).next().is_some_and(|name|name.eq_ignore_ascii_case("mstsc.exe"))
    }
    fn same_rect(a:&RECT,b:&RECT)->bool {
        a.left==b.left && a.top==b.top && a.right==b.right && a.bottom==b.bottom
    }
    unsafe fn place(hwnd:HWND,rc:&RECT)->bool {
        SetWindowPos(hwnd,null_mut(),rc.left,rc.top,rc.right-rc.left,rc.bottom-rc.top,SWP_NOACTIVATE|SWP_NOZORDER)!=0
    }
    pub unsafe fn reconcile() {
        let (enabled, has_incidents, active_monitors)={
            let ui=UI.get().unwrap().lock().unwrap();
            (ui.display.rdp_compat_mode,!ui.incidents.is_empty(),
             ui.windows.iter().filter(|w|w.registered)
               .map(|w|w.monitor.bounds).collect::<Vec<_>>())
        };
        let saved=ADJUSTED.get_or_init(||Mutex::new(HashMap::new()));
        let mut saved=saved.lock().unwrap();
        if !enabled || !has_incidents {
            for (id,adj) in saved.drain() {
                let hwnd=id as HWND;let mut current:RECT=zeroed();
                // Restore only if the user has not moved or resized the window.
                if is_window(hwnd)!=0 && window_rect(hwnd,&mut current)!=0 && same_rect(&current,&adj.applied){place(hwnd,&adj.original);}
            }
            return;
        }
        if active_monitors.is_empty(){return}
        let mut candidates:Vec<HWND>=Vec::new();
        enum_windows(Some(collect),&mut candidates as *mut _ as LPARAM);
        let mut seen=HashSet::new();
        for hwnd in candidates {
            if !mstsc(hwnd){continue}
            if !saved.contains_key(&(hwnd as usize)) && zoomed(hwnd)==0 {continue}
            let id=hwnd as usize;seen.insert(id);
            let mon=monitor_from_window(hwnd,2); // MONITOR_DEFAULTTONEAREST
            if mon.is_null(){continue}
            let mut mi:MONITORINFO=zeroed();mi.cbSize=size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(mon,&mut mi)==0 {continue}
            if !active_monitors.iter().any(|b|same_rect(b,&mi.rcMonitor)){continue}
            let mut current:RECT=zeroed();if window_rect(hwnd,&mut current)==0 {continue}
            let work=mi.rcWork;
            // Only correct an overlap with the appbar. Other programs and
            // fullscreen windows not reported as maximized are left alone.
            let target=RECT{left:work.left,top:work.top,right:work.right,bottom:work.bottom};
            if target.right<=target.left || target.bottom<=target.top {continue}
            if let Some(old)=saved.get_mut(&id) {
                if !same_rect(&current,&old.applied) && !same_rect(&current,&old.original) {
                    // User changed the window; release ownership, do not fight it.
                    saved.remove(&id);continue;
                }
                if !same_rect(&old.applied,&target) && place(hwnd,&target){old.applied=target;}
            } else {
                let overlap=current.top<work.top || current.bottom>work.bottom
                    || current.left<work.left || current.right>work.right;
                if overlap && place(hwnd,&target) {saved.insert(id,Adjusted{original:current,applied:target});}
            }
        }
        saved.retain(|id,_|seen.contains(id));
    }
}

const UPDATE_MESSAGE:u32 = WM_APP + 12;
const APPBAR_MESSAGE:u32 = WM_APP + 13;
const TIMER_ID:usize = 1;

#[derive(Clone, Deserialize)]
struct Config {server_url:String, client_token:String, poll_seconds:Option<u64>}
#[derive(Clone, Deserialize, Default)]
struct Incident {#[serde(default)] id:String, title:String, message:String, severity:String, status:String}
#[derive(Clone, Deserialize)]
#[serde(default)]
struct DisplaySettings {position:String, all_monitors:bool, height_px:i32, initial_height_px:i32, initial_seconds:u64, #[serde(default)] rdp_compat_mode:bool}
impl Default for DisplaySettings {
    fn default()->Self {Self {position:"bottom".into(), all_monitors:true, height_px:44, initial_height_px:88, initial_seconds:5, rdp_compat_mode:false}}
}
impl DisplaySettings {
    fn normalized(mut self)->Self {
        if self.position != "top" {self.position="bottom".into();}
        self.height_px=self.height_px.clamp(24,160);
        self.initial_height_px=self.initial_height_px.clamp(self.height_px,300);
        self.initial_seconds=self.initial_seconds.min(60);
        self
    }
}
#[derive(Deserialize)]
struct ClientState {incidents:Vec<Incident>, #[serde(default)] display:DisplaySettings}
#[derive(Clone, Copy)]
struct Monitor {bounds:RECT, primary:bool}
#[derive(Clone, Copy)]
struct WindowInfo {hwnd:usize, monitor:Monitor, registered:bool, height:i32, at_top:bool}
struct Ui {
    incidents:Vec<Incident>, index:usize, display:DisplaySettings,
    windows:Vec<WindowInfo>, emphasized_until:Option<Instant>,
    last_rotation:Instant,
}
static UI:OnceLock<Mutex<Ui>>=OnceLock::new();
static PENDING:OnceLock<Mutex<Option<ClientState>>>=OnceLock::new();
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(std::iter::once(0)).collect()}
fn config_path()->PathBuf{
    let base=env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(||PathBuf::from("C:\\ProgramData"));
    let path=base.join("VGStatusbar").join("client.json");
    if path.exists(){path}else{env::current_exe().unwrap_or_default().with_file_name("client.json")}
}
fn username()->String {let mut buf=[0u16;256];let mut len=256u32;unsafe {if GetUserNameW(buf.as_mut_ptr(),&mut len)!=0{return String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize])}};String::new()}
fn computer()->String {let mut buf=[0u16;256];let mut len=256u32;unsafe {if GetComputerNameW(buf.as_mut_ptr(),&mut len)!=0{return String::from_utf16_lossy(&buf[..len as usize])}};String::new()}
fn worker(hwnd:HWND,cfg:Config){
    let user=username();let computer=computer();
    let agent=match reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build(){Ok(c)=>c,Err(_)=>return};
    loop {
        let url=format!("{}/api/client/state",cfg.server_url.trim_end_matches('/'));
        let result=agent.get(url).bearer_auth(&cfg.client_token).query(&[("user",user.as_str()),("computer",computer.as_str())]).send()
            .and_then(|r|r.error_for_status()).and_then(|r|r.json::<ClientState>());
        // Keep the last known state when offline; never invent an all-clear.
        if let Ok(state)=result {
            if let Ok(mut pending)=PENDING.get().expect("pending").lock(){*pending=Some(state);}
            unsafe {PostMessageW(hwnd,UPDATE_MESSAGE,0,0);}
        }
        thread::sleep(Duration::from_secs(cfg.poll_seconds.unwrap_or(10).clamp(5,300)));
    }
}
unsafe extern "system" fn enum_monitor(hmon:HMONITOR,_hdc:HDC,_rect:*mut RECT,param:LPARAM)->i32 {
    let monitors=&mut *(param as *mut Vec<Monitor>);
    let mut info:MONITORINFO=zeroed(); info.cbSize=size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(hmon,&mut info)!=0 {
        monitors.push(Monitor{bounds:info.rcMonitor,primary:(info.dwFlags & 1u32)!=0});
    }
    1
}
unsafe fn monitors()->Vec<Monitor> {
    let mut list:Vec<Monitor>=Vec::new();
    EnumDisplayMonitors(null_mut(),null(),Some(enum_monitor),&mut list as *mut _ as LPARAM);
    list.sort_by_key(|m|if m.primary {0}else{1});
    list
}
unsafe fn appbar_data(hwnd:HWND,at_top:bool)->APPBARDATA {
    APPBARDATA {cbSize:size_of::<APPBARDATA>() as u32,hWnd:hwnd,uCallbackMessage:APPBAR_MESSAGE,
        uEdge:if at_top {ABE_TOP}else{ABE_BOTTOM},rc:RECT{left:0,top:0,right:0,bottom:0},lParam:0}
}
unsafe fn layout(force:bool) {
    let (windows,show,display,height,at_top)={
        let ui=UI.get().unwrap().lock().unwrap();
        let big=ui.emphasized_until.is_some_and(|until|Instant::now()<until);
        (ui.windows.clone(),!ui.incidents.is_empty(),ui.display.clone(),
            if big {ui.display.initial_height_px}else{ui.display.height_px},ui.display.position=="top")
    };
    let mut updates=Vec::new();
    for w in windows {
        let hwnd=w.hwnd as HWND;
        let enabled=show && (display.all_monitors || w.monitor.primary);
        if !enabled {
            if w.registered {
                let mut d=appbar_data(hwnd,w.at_top); SHAppBarMessage(ABM_REMOVE,&mut d);
            }
            ShowWindow(hwnd,SW_HIDE);
            updates.push((w.hwnd,false,0,at_top));
            continue;
        }
        let mut registered=w.registered;
        // Re-registration is necessary when switching between top and bottom.
        if registered && w.at_top!=at_top {
            let mut old=appbar_data(hwnd,w.at_top);SHAppBarMessage(ABM_REMOVE,&mut old);
            registered=false;
        }
        if !registered {
            let mut d=appbar_data(hwnd,at_top);
            registered=SHAppBarMessage(ABM_NEW,&mut d)!=0;
        }
        if registered && (force || !w.registered || w.height!=height || w.at_top!=at_top) {
            let mut d=appbar_data(hwnd,at_top);
            d.rc=w.monitor.bounds;
            SHAppBarMessage(ABM_QUERYPOS,&mut d);
            if at_top {d.rc.bottom=d.rc.top+height;} else {d.rc.top=d.rc.bottom-height;}
            SHAppBarMessage(ABM_SETPOS,&mut d);
            SetWindowPos(hwnd,null_mut(),d.rc.left,d.rc.top,d.rc.right-d.rc.left,d.rc.bottom-d.rc.top,
                SWP_NOACTIVATE|SWP_SHOWWINDOW|SWP_NOZORDER);
            InvalidateRect(hwnd,null(),0);
        }
        updates.push((w.hwnd,registered,height,at_top));
    }
    let mut ui=UI.get().unwrap().lock().unwrap();
    for (hwnd,registered,height,at_top) in updates {
        if let Some(w)=ui.windows.iter_mut().find(|w|w.hwnd==hwnd) {
            w.registered=registered;w.height=height;w.at_top=at_top;
        }
    }
}
unsafe fn redraw_all(){
    let windows={UI.get().unwrap().lock().unwrap().windows.clone()};
    for w in windows {InvalidateRect(w.hwnd as HWND,null(),0);}
}
unsafe fn update_client_state(){
    let next=match PENDING.get().unwrap().lock().unwrap().take(){Some(s)=>s,None=>return};
    let cfg=next.display.normalized();
    {
        let mut ui=UI.get().unwrap().lock().unwrap();
        let previous:HashSet<String>=ui.incidents.iter().filter(|i|i.status=="active")
            .map(|i|i.id.clone()).collect();
        let new_active=next.incidents.iter().filter(|i|i.status=="active")
            .any(|i|!previous.contains(&i.id));
        if new_active && cfg.initial_seconds>0 {
            ui.emphasized_until=Some(Instant::now()+Duration::from_secs(cfg.initial_seconds));
        }
        if next.incidents.is_empty(){ui.emphasized_until=None;}
        if ui.incidents.get(ui.index).map(|i|i.id.as_str()) !=next.incidents.get(ui.index).map(|i|i.id.as_str()) {
            ui.index=0;
        }
        ui.incidents=next.incidents;
        ui.display=cfg;
    }
    layout(false);redraw_all();rdp::reconcile();
}
unsafe fn tick(){
    let mut need_layout=false;
    let mut rotate=false;
    {
        let mut ui=UI.get().unwrap().lock().unwrap();
        if ui.emphasized_until.is_some_and(|until|Instant::now()>=until) {
            ui.emphasized_until=None;need_layout=true;
        }
        if ui.last_rotation.elapsed()>=Duration::from_secs(7){
            if ui.incidents.len()>1 {ui.index=(ui.index+1)%ui.incidents.len();rotate=true;}
            ui.last_rotation=Instant::now();
        }
    }
    if need_layout {layout(false);}
    if rotate {redraw_all();}
    rdp::reconcile();
}
unsafe extern "system" fn wndproc(hwnd:HWND,msg:u32,wp:WPARAM,lp:LPARAM)->LRESULT {
    match msg {
        UPDATE_MESSAGE=>{update_client_state();0},
        WM_TIMER=>{if wp==TIMER_ID {tick();}0},
        WM_DISPLAYCHANGE|WM_SETTINGCHANGE=>{layout(true);rdp::reconcile();0},
        APPBAR_MESSAGE=>{if wp==1 {layout(true);}0},
        WM_PAINT=>{
            let mut ps:PAINTSTRUCT=zeroed();let hdc=BeginPaint(hwnd,&mut ps);
            if !hdc.is_null(){
                let mut rect:RECT=zeroed();
                windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd,&mut rect);
                let (incident,big)={let ui=UI.get().unwrap().lock().unwrap();
                    (ui.incidents.get(ui.index).cloned(),ui.emphasized_until.is_some_and(|t|Instant::now()<t))};
                if let Some(i)=incident {
                    let color=if i.status=="resolved" {0x004C7D26} else if i.severity=="warning" {0x000092D2}
                        else if i.severity=="info" {0x00A46A30} else {0x002B2BA5};
                    let brush=CreateSolidBrush(color);FillRect(hdc,&rect,brush);DeleteObject(brush);
                    SetTextColor(hdc,0x00FFFFFF);SetBkMode(hdc,TRANSPARENT as i32);
                    let prefix=if i.status=="resolved" {"ENTWARNUNG"} else if i.severity=="warning" {"EINSCHRAENKUNG"}
                        else if i.severity=="info" {"HINWEIS"} else {"IT-STOERUNG"};
                    let text=wide(&format!("{}: {}  -  {}",prefix,i.title,i.message.replace('\r'," ").replace('\n'," ")));
                    let mut font:LOGFONTW=zeroed();
                    font.lfHeight=if big {-30}else{-18};font.lfWeight=700;
                    for (dst,src) in font.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()){*dst=src;}
                    let hfont=CreateFontIndirectW(&font);
                    let old=if !hfont.is_null(){SelectObject(hdc,hfont)}else{null_mut()};
                    DrawTextW(hdc,text.as_ptr(),-1,&mut rect,DT_CENTER|DT_VCENTER|DT_SINGLELINE);
                    if !hfont.is_null(){SelectObject(hdc,old);DeleteObject(hfont);}
                }
                EndPaint(hwnd,&ps);
            }
            0
        },
        WM_CLOSE=>{DestroyWindow(hwnd);0},
        WM_DESTROY=>{
            let win={UI.get().unwrap().lock().unwrap().windows.iter().find(|w|w.hwnd==hwnd as usize).copied()};
            if let Some(w)=win {if w.registered {let mut d=appbar_data(hwnd,w.at_top);SHAppBarMessage(ABM_REMOVE,&mut d);}}
            // The first window owns the timer and message loop.
            let primary={UI.get().unwrap().lock().unwrap().windows.first().map(|w|w.hwnd)};
            if primary==Some(hwnd as usize){KillTimer(hwnd,TIMER_ID);PostQuitMessage(0);}
            0
        },
        _=>DefWindowProcW(hwnd,msg,wp,lp),
    }
}
pub fn main(){
    let path=config_path();
    // Ignore an optional UTF-8 BOM from Windows PowerShell 5.1.
    let cfg:Config=match fs::read_to_string(&path).ok().and_then(|s|serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()){
        Some(c)=>c,None=>{eprintln!("Missing or invalid config: {}",path.display());return}
    };
    if !cfg.server_url.starts_with("https://") && !cfg.server_url.starts_with("http://127.0.0.1") && !cfg.server_url.starts_with("http://localhost") {return;}
    UI.set(Mutex::new(Ui{incidents:vec![],index:0,display:DisplaySettings::default(),
        windows:vec![],emphasized_until:None,last_rotation:Instant::now()})).ok();
    PENDING.set(Mutex::new(None)).ok();
    unsafe {
        let name=wide("VGStatusbarRustWindow");
        let instance=GetModuleHandleW(null());
        let wc=WNDCLASSW{style:0,lpfnWndProc:Some(wndproc),cbClsExtra:0,cbWndExtra:0,hInstance:instance,
            hIcon:null_mut(),hCursor:LoadCursorW(null_mut(),IDC_ARROW),hbrBackground:null_mut(),
            lpszMenuName:null(),lpszClassName:name.as_ptr()};
        if RegisterClassW(&wc)==0 {return;}
        let title=wide("VG Statusbar");
        let monitors=monitors();
        for monitor in monitors {
            let hwnd=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_TOPMOST|WS_EX_NOACTIVATE,
                name.as_ptr(),title.as_ptr(),WS_POPUP,monitor.bounds.left,monitor.bounds.top,100,44,
                null_mut(),null_mut(),instance,null());
            if !hwnd.is_null(){UI.get().unwrap().lock().unwrap().windows.push(WindowInfo{
                hwnd:hwnd as usize,monitor,registered:false,height:0,at_top:false});}
        }
        let first={UI.get().unwrap().lock().unwrap().windows.first().map(|w|w.hwnd)};
        let Some(first)=first else{return};
        let hwnd=first as HWND;
        let hwnd_bits=hwnd as usize;
        thread::spawn(move||worker(hwnd_bits as HWND,cfg));
        SetTimer(hwnd,TIMER_ID,1000,None);
        let mut message:MSG=zeroed();
        while GetMessageW(&mut message,null_mut(),0,0)>0 {TranslateMessage(&message);DispatchMessageW(&message);}
    }
}
}
#[cfg(windows)]
fn main(){win::main()}
