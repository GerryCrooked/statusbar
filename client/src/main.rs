#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(not(windows))]
fn main() { eprintln!("This desktop client is Windows-only."); }

#[cfg(windows)]
mod win {
use std::{env, fs, mem::{size_of, zeroed}, path::PathBuf, ptr::{null, null_mut}, sync::{Mutex, OnceLock}, thread, time::Duration};
use serde::Deserialize;
use windows_sys::Win32::{
 Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
 Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor, PAINTSTRUCT, TRANSPARENT, DrawTextW, InvalidateRect, DT_CENTER, DT_SINGLELINE, DT_VCENTER},
 System::{LibraryLoader::GetModuleHandleW, WindowsProgramming::{GetComputerNameW,GetUserNameW}},
 UI::{Shell::{APPBARDATA, ABM_NEW, ABM_REMOVE, ABM_QUERYPOS, ABM_SETPOS, ABE_BOTTOM, SHAppBarMessage},
 WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WNDCLASSW, MSG, WM_APP, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_PAINT, WM_SETTINGCHANGE, WM_TIMER, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SWP_NOZORDER, IDC_ARROW},
 },
};

const BAR_HEIGHT: i32=44;
const UPDATE_MESSAGE: u32=WM_APP+12;
const APPBAR_MESSAGE: u32=WM_APP+13;
#[derive(Clone,Deserialize)]
struct Config { server_url:String, client_token:String, poll_seconds:Option<u64> }
#[derive(Clone,Deserialize,Default)]
struct Incident { title:String, message:String, severity:String, status:String }
#[derive(Deserialize)]
struct State { incidents:Vec<Incident> }
struct Ui { incidents:Vec<Incident>, index:usize, registered:bool }
static UI:OnceLock<Mutex<Ui>>=OnceLock::new();
static PENDING:OnceLock<Mutex<Vec<Incident>>>=OnceLock::new();
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(std::iter::once(0)).collect()}
fn config_path()->PathBuf{
    let base=env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(||PathBuf::from("C:\\ProgramData"));
    let path=base.join("VGStatusbar").join("client.json");
    if path.exists(){path}else{env::current_exe().unwrap_or_default().with_file_name("client.json")}
}
fn username()->String {let mut buf=[0u16;256];let mut len=256u32;unsafe {if GetUserNameW(buf.as_mut_ptr(),&mut len)!=0{return String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize])}};String::new()}
fn computer()->String {let mut buf=[0u16;256];let mut len=256u32;unsafe {if GetComputerNameW(buf.as_mut_ptr(),&mut len)!=0{return String::from_utf16_lossy(&buf[..len as usize])}};String::new()}
fn worker(hwnd:HWND, cfg:Config){
    let user=username();let computer=computer();
    let agent=match reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build(){Ok(c)=>c,Err(_)=>return};
    loop {
        let url=format!("{}/api/client/state",cfg.server_url.trim_end_matches('/'));
        let result=agent.get(url).bearer_auth(&cfg.client_token).query(&[("user",user.as_str()),("computer",computer.as_str())]).send()
            .and_then(|r|r.error_for_status()).and_then(|r|r.json::<State>());
        // During a connectivity failure keep last known incidents; don't announce fake resolution.
        if let Ok(state)=result {
            if let Ok(mut p)=PENDING.get().expect("pending").lock(){*p=state.incidents;}
            unsafe {PostMessageW(hwnd,UPDATE_MESSAGE,0,0);}
        }
        thread::sleep(Duration::from_secs(cfg.poll_seconds.unwrap_or(10).clamp(5,300)));
    }
}
unsafe fn appbar_data(hwnd:HWND)->APPBARDATA {
    APPBARDATA {cbSize:size_of::<APPBARDATA>() as u32,hWnd:hwnd,uCallbackMessage:APPBAR_MESSAGE,uEdge:ABE_BOTTOM,rc:RECT{left:0,top:0,right:0,bottom:0},lParam:0}
}
unsafe fn reposition(hwnd:HWND) {
    // Full primary display; shell adjusts the proposed rectangle to avoid the real taskbar.
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics,SM_CXSCREEN,SM_CYSCREEN};
    let width=GetSystemMetrics(SM_CXSCREEN);
    let height=GetSystemMetrics(SM_CYSCREEN);
    let mut data=appbar_data(hwnd);
    data.rc=RECT{left:0,top:0,right:width,bottom:height};
    SHAppBarMessage(ABM_QUERYPOS,&mut data);
    data.rc.top=data.rc.bottom-BAR_HEIGHT;
    SHAppBarMessage(ABM_SETPOS,&mut data);
    SetWindowPos(hwnd,null_mut(),data.rc.left,data.rc.top,data.rc.right-data.rc.left,BAR_HEIGHT,SWP_NOACTIVATE|SWP_SHOWWINDOW|SWP_NOZORDER);
}
unsafe fn set_visibility(hwnd:HWND,show:bool){
    let mut ui=UI.get().unwrap().lock().unwrap();
    if show && !ui.registered {
        let mut d=appbar_data(hwnd);
        if SHAppBarMessage(ABM_NEW,&mut d)!=0 {ui.registered=true;drop(ui);reposition(hwnd);ShowWindow(hwnd,SW_SHOWNOACTIVATE);}
    } else if !show && ui.registered {
        let mut d=appbar_data(hwnd);SHAppBarMessage(ABM_REMOVE,&mut d);ui.registered=false;
        drop(ui);ShowWindow(hwnd,SW_HIDE);
    }
}
unsafe extern "system" fn wndproc(hwnd:HWND,msg:u32,wp:WPARAM,lp:LPARAM)->LRESULT {
    match msg {
        UPDATE_MESSAGE=>{
            let next=PENDING.get().unwrap().lock().unwrap().clone();
            let show=!next.is_empty();
            {let mut ui=UI.get().unwrap().lock().unwrap(); ui.incidents=next;ui.index=0;}
            set_visibility(hwnd,show);
            InvalidateRect(hwnd,null(),0);
            0
        },
        WM_TIMER=>{
            if let Ok(mut ui)=UI.get().unwrap().lock(){if !ui.incidents.is_empty(){ui.index=(ui.index+1)%ui.incidents.len();}}
            InvalidateRect(hwnd,null(),0);0
        },
        WM_DISPLAYCHANGE|WM_SETTINGCHANGE=>{
            if UI.get().unwrap().lock().unwrap().registered {reposition(hwnd)};0
        },
        APPBAR_MESSAGE=>{
            // ABN_POSCHANGED = 1, shell asks us to update our reserved rectangle.
            if wp==1 && UI.get().unwrap().lock().unwrap().registered {reposition(hwnd)};0
        },
        WM_PAINT=>{
            let mut ps:PAINTSTRUCT=zeroed();let hdc=BeginPaint(hwnd,&mut ps);
            if !hdc.is_null(){
                let mut rect:RECT=zeroed();GetClientRect(hwnd,&mut rect);
                let incident={let ui=UI.get().unwrap().lock().unwrap();ui.incidents.get(ui.index).cloned()};
                if let Some(i)=incident {
                    let color=if i.status=="resolved" {0x004C7D26} else if i.severity=="warning" {0x000092D2} else if i.severity=="info" {0x00A46A30} else {0x002B2BA5};
                    let brush=CreateSolidBrush(color);FillRect(hdc,&rect,brush);DeleteObject(brush);
                    SetTextColor(hdc,0x00FFFFFF);SetBkMode(hdc,TRANSPARENT as i32);
                    let prefix=if i.status=="resolved" {"ENTWARNUNG"} else if i.severity=="warning" {"EINSCHRAENKUNG"} else if i.severity=="info" {"HINWEIS"} else {"IT-STOERUNG"};
                    let text=wide(&format!("{}: {}  -  {}",prefix,i.title,i.message.replace('\r'," ").replace('\n'," ")));
                    DrawTextW(hdc,text.as_ptr(),-1,&mut rect,DT_CENTER|DT_VCENTER|DT_SINGLELINE);
                }
                EndPaint(hwnd,&ps);
            }
            0
        },
        WM_CLOSE=>{DestroyWindow(hwnd);0},
        WM_DESTROY=>{
            let registered=UI.get().unwrap().lock().unwrap().registered;
            if registered {let mut d=appbar_data(hwnd);SHAppBarMessage(ABM_REMOVE,&mut d);}
            KillTimer(hwnd,1);PostQuitMessage(0);0
        },
        _=>DefWindowProcW(hwnd,msg,wp,lp),
    }
}
pub fn main(){
    let path=config_path();
    let cfg:Config=match fs::read_to_string(&path).ok().and_then(|x|serde_json::from_str(&x).ok()){
        Some(c)=>c,None=>{eprintln!("Missing config: {}",path.display());return}
    };
    if !cfg.server_url.starts_with("https://") && !cfg.server_url.starts_with("http://127.0.0.1") && !cfg.server_url.starts_with("http://localhost") {return;}
    UI.set(Mutex::new(Ui{incidents:vec![],index:0,registered:false})).ok();
    PENDING.set(Mutex::new(vec![])).ok();
    unsafe {
        let name=wide("VGStatusbarRustWindow");
        let instance=GetModuleHandleW(null());
        let wc=WNDCLASSW{style:0,lpfnWndProc:Some(wndproc),cbClsExtra:0,cbWndExtra:0,hInstance:instance,hIcon:null_mut(),hCursor:LoadCursorW(null_mut(),IDC_ARROW),hbrBackground:null_mut(),lpszMenuName:null(),lpszClassName:name.as_ptr()};
        if RegisterClassW(&wc)==0{return;}
        let title=wide("VG Statusbar");
        let hwnd=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_TOPMOST,name.as_ptr(),title.as_ptr(),WS_POPUP,0,0,100,BAR_HEIGHT,null_mut(),null_mut(),instance,null());
        if hwnd.is_null(){return;}
        let hwnd_bits=hwnd as usize;
        thread::spawn(move||worker(hwnd_bits as HWND,cfg));
        SetTimer(hwnd,1,7500,None);
        let mut message:MSG=zeroed();
        while GetMessageW(&mut message,null_mut(),0,0)>0{TranslateMessage(&message);DispatchMessageW(&message);}
    }
}
}
#[cfg(windows)]
fn main(){win::main()}
